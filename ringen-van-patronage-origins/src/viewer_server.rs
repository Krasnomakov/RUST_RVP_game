use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
};

const ADDRESS: &str = "127.0.0.1:4173";
const MAX_REQUEST_BODY: usize = 512 * 1024;
const LEVEL_FILES: [&str; 6] = [
    "data/levels/level-001.json",
    "data/levels/level-002.json",
    "data/levels/level-003.json",
    "data/levels/level-004.json",
    "data/levels/level-005.json",
    "data/levels/level-006.json",
];

pub(crate) fn run(project_root: PathBuf) {
    let listener = match TcpListener::bind(ADDRESS) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("Could not start the learning viewer at http://{ADDRESS}: {error}");
            return;
        }
    };
    println!("Learning studio running at http://{ADDRESS}");
    serve(listener, project_root);
}

pub(crate) fn start(project_root: PathBuf) -> std::io::Result<()> {
    let listener = TcpListener::bind(ADDRESS)?;
    println!("Learning studio running at http://{ADDRESS}");
    thread::spawn(move || serve(listener, project_root));
    Ok(())
}

fn serve(listener: TcpListener, project_root: PathBuf) {
    let build_state = BuildState::new(&project_root);
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let root = project_root.clone();
                let state = build_state.clone();
                thread::spawn(move || handle_connection(stream, &root, &state));
            }
            Err(error) => eprintln!("Viewer connection failed: {error}"),
        }
    }
}

struct HttpRequest {
    method: String,
    route: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn handle_connection(mut stream: TcpStream, project_root: &Path, build_state: &BuildState) {
    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err(status) => {
            let result = response(status, "text/plain; charset=utf-8", b"Invalid request");
            let _ = stream.write_all(&result);
            return;
        }
    };

    if request.method != "GET" && !is_local_origin(&request.headers) {
        let result = response(403, "text/plain; charset=utf-8", b"Local origin required");
        let _ = stream.write_all(&result);
        return;
    }

    let method = request.method.as_str();
    let route = request.route.as_str();
    let result = match (method, route) {
        ("GET", "/") | ("GET", "/index.html") => response(
            200,
            "text/html; charset=utf-8",
            include_bytes!("../viewer/index.html"),
        ),
        ("GET", "/styles.css") => response(
            200,
            "text/css; charset=utf-8",
            include_bytes!("../viewer/styles.css"),
        ),
        ("GET", "/app.js") => response(
            200,
            "text/javascript; charset=utf-8",
            include_bytes!("../viewer/app.js"),
        ),
        ("GET", "/game/") | ("GET", "/game") | ("GET", "/game/index.html") => response(
            200,
            "text/html; charset=utf-8",
            include_bytes!("../viewer/game.html"),
        ),
        ("GET", "/game/loader.js") => response(
            200,
            "text/javascript; charset=utf-8",
            include_bytes!("../viewer/game-loader.js"),
        ),
        ("GET", "/api/game/build") => build_status_response(build_state),
        ("GET", route) if route.starts_with("/game-build/") => {
            game_build_file_response(project_root, route)
        }
        ("GET", "/api/levels") => levels_response(project_root),
        ("GET", route) if route.starts_with("/api/levels/") => level_response(project_root, route),
        ("PUT", route) if route.starts_with("/api/levels/") && route.ends_with("/source") => {
            save_source_response(project_root, route, &request.body)
        }
        ("POST", "/api/review") => review_response(project_root),
        ("POST", "/api/game/build") => start_game_build(project_root.to_path_buf(), build_state),
        ("GET", route) if route.starts_with("/reading/") => {
            reading_response(project_root, route.trim_start_matches("/reading/"))
        }
        ("GET", _) | ("POST", _) | ("PUT", _) => {
            response(404, "text/plain; charset=utf-8", b"Not found")
        }
        _ => response(405, "text/plain; charset=utf-8", b"Method not allowed"),
    };

    let _ = stream.write_all(&result);
    let _ = stream.flush();
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, u16> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    let (header_end, body_length) = loop {
        let count = stream.read(&mut chunk).map_err(|_| 400_u16)?;
        if count == 0 {
            return Err(400);
        }
        bytes.extend_from_slice(&chunk[..count]);
        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            let header_end = index + 4;
            let head = String::from_utf8_lossy(&bytes[..index]);
            let length = head
                .lines()
                .skip(1)
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if length > MAX_REQUEST_BODY {
                return Err(413);
            }
            if bytes.len() >= header_end + length {
                break (header_end, length);
            }
            while bytes.len() < header_end + length {
                let count = stream.read(&mut chunk).map_err(|_| 400_u16)?;
                if count == 0 {
                    return Err(400);
                }
                bytes.extend_from_slice(&chunk[..count]);
            }
            break (header_end, length);
        }
        if bytes.len() > MAX_REQUEST_BODY + 16 * 1024 {
            return Err(413);
        }
    };

    let head = String::from_utf8_lossy(&bytes[..header_end - 4]);
    let mut lines = head.lines();
    let mut request_line = lines.next().unwrap_or_default().split_whitespace();
    let method = request_line.next().ok_or(400_u16)?.to_string();
    let route = request_line
        .next()
        .ok_or(400_u16)?
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect();
    let body = bytes[header_end..header_end + body_length].to_vec();
    Ok(HttpRequest {
        method,
        route,
        headers,
        body,
    })
}

fn is_local_origin(headers: &[(String, String)]) -> bool {
    headers
        .iter()
        .find(|(name, _)| name == "origin")
        .is_some_and(|(_, origin)| {
            origin == "http://127.0.0.1:4173" || origin == "http://localhost:4173"
        })
}

#[derive(Clone)]
struct BuildState(Arc<Mutex<BuildSnapshot>>);

#[derive(Default)]
struct BuildSnapshot {
    running: bool,
    available: bool,
    success: Option<bool>,
    output: String,
}

impl BuildState {
    fn new(project_root: &Path) -> Self {
        let output_dir = project_root.join("target/game-web");
        let available = output_dir.join("ringen_game.js").is_file()
            && output_dir.join("ringen_game_bg.wasm").is_file();
        Self(Arc::new(Mutex::new(BuildSnapshot {
            available,
            ..Default::default()
        })))
    }
}

fn build_status_response(state: &BuildState) -> Vec<u8> {
    let snapshot = state.0.lock().unwrap_or_else(|error| error.into_inner());
    json_response(
        200,
        &json!({
            "running": snapshot.running,
            "available": snapshot.available,
            "success": snapshot.success,
            "output": snapshot.output,
        }),
    )
}

fn start_game_build(project_root: PathBuf, state: &BuildState) -> Vec<u8> {
    {
        let mut snapshot = state.0.lock().unwrap_or_else(|error| error.into_inner());
        if snapshot.running {
            return json_response(
                409,
                &json!({ "error": "A browser build is already running." }),
            );
        }
        snapshot.running = true;
        snapshot.success = None;
        snapshot.output = "Starting WebAssembly build…".to_string();
    }

    let state = state.clone();
    thread::spawn(move || {
        let (success, output) = compile_browser_game(&project_root);
        let available = project_root
            .join("target/game-web/ringen_game.js")
            .is_file()
            && project_root
                .join("target/game-web/ringen_game_bg.wasm")
                .is_file();
        let mut snapshot = state.0.lock().unwrap_or_else(|error| error.into_inner());
        snapshot.running = false;
        snapshot.available = available;
        snapshot.success = Some(success);
        snapshot.output = output;
    });

    json_response(202, &json!({ "running": true }))
}

fn compile_browser_game(project_root: &Path) -> (bool, String) {
    let build = Command::new("cargo")
        .args([
            "build",
            "--target",
            "wasm32-unknown-unknown",
            "--bin",
            "ringen_van_patronage_origins",
        ])
        .current_dir(project_root)
        .output();
    let build = match build {
        Ok(output) => output,
        Err(error) => return (false, format!("Could not start cargo: {error}")),
    };
    let mut details = format!(
        "{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    if !build.status.success() {
        return (false, truncate(&details, 12_000));
    }

    let input =
        project_root.join("target/wasm32-unknown-unknown/debug/ringen_van_patronage_origins.wasm");
    let output_dir = project_root.join("target/game-web");
    if let Err(error) = fs::create_dir_all(&output_dir) {
        details.push_str(&format!(
            "\nCould not create browser output folder: {error}"
        ));
        return (false, truncate(&details, 12_000));
    }
    let bindgen = Command::new("wasm-bindgen")
        .args(["--target", "web", "--out-name", "ringen_game"])
        .arg("--out-dir")
        .arg(&output_dir)
        .arg(&input)
        .current_dir(project_root)
        .output();
    let bindgen = match bindgen {
        Ok(output) => output,
        Err(error) => {
            details.push_str(&format!("\nCould not start wasm-bindgen: {error}"));
            return (false, truncate(&details, 12_000));
        }
    };
    details.push_str(&String::from_utf8_lossy(&bindgen.stdout));
    details.push_str(&String::from_utf8_lossy(&bindgen.stderr));
    (bindgen.status.success(), truncate(&details, 12_000))
}

fn game_build_file_response(project_root: &Path, route: &str) -> Vec<u8> {
    let filename = route.trim_start_matches("/game-build/");
    if !matches!(filename, "ringen_game.js" | "ringen_game_bg.wasm") {
        return response(
            404,
            "text/plain; charset=utf-8",
            b"Game build file not found",
        );
    }
    let path = project_root.join("target/game-web").join(filename);
    match fs::read(&path) {
        Ok(contents) => response(200, content_type(&path), &contents),
        Err(_) => response(404, "text/plain; charset=utf-8", b"Game build is not ready"),
    }
}

fn save_source_response(project_root: &Path, route: &str, body: &[u8]) -> Vec<u8> {
    let parts = route
        .trim_start_matches("/api/levels/")
        .split('/')
        .collect::<Vec<_>>();
    if parts.len() != 2 || parts[1] != "source" {
        return response(404, "text/plain; charset=utf-8", b"Unknown source file");
    }
    let Some(relative_file) = level_file(parts[0]) else {
        return response(404, "text/plain; charset=utf-8", b"Unknown level");
    };
    let metadata = match read_json(project_root.join(relative_file)) {
        Ok(metadata) => metadata,
        Err(error) => return json_response(500, &json!({ "error": error.to_string() })),
    };
    let Some(source_path) = metadata.get("source_path").and_then(Value::as_str) else {
        return response(500, "text/plain; charset=utf-8", b"Source path is missing");
    };
    let relative = Path::new(source_path);
    if relative
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("rs")
        || !relative.starts_with("src")
    {
        return response(
            403,
            "text/plain; charset=utf-8",
            b"Only Rust source files can be edited",
        );
    }
    let Some(path) = safe_project_file(project_root, relative) else {
        return response(
            403,
            "text/plain; charset=utf-8",
            b"Source path is not allowed",
        );
    };
    let payload: Value = match serde_json::from_slice(body) {
        Ok(payload) => payload,
        Err(error) => return json_response(400, &json!({ "error": error.to_string() })),
    };
    let Some(source) = payload.get("source").and_then(Value::as_str) else {
        return response(400, "text/plain; charset=utf-8", b"Source text is missing");
    };
    if let Err(error) = fs::write(&path, source) {
        return json_response(500, &json!({ "error": error.to_string() }));
    }
    json_response(200, &json!({ "saved": true, "source_path": source_path }))
}

fn levels_response(project_root: &Path) -> Vec<u8> {
    let levels = LEVEL_FILES
        .iter()
        .filter_map(|relative| read_json(project_root.join(relative)).ok())
        .collect::<Vec<_>>();
    json_response(200, &Value::Array(levels))
}

fn level_response(project_root: &Path, route: &str) -> Vec<u8> {
    let parts = route
        .trim_start_matches("/api/levels/")
        .split('/')
        .collect::<Vec<_>>();
    let Some(relative_file) = parts.first().and_then(|id| level_file(id)) else {
        return response(404, "text/plain; charset=utf-8", b"Unknown level");
    };
    let metadata = match read_json(project_root.join(relative_file)) {
        Ok(metadata) => metadata,
        Err(error) => {
            return json_response(500, &json!({ "error": error.to_string() }));
        }
    };

    match parts.as_slice() {
        [_, "source"] => {
            let Some(source_path) = metadata.get("source_path").and_then(Value::as_str) else {
                return response(500, "text/plain; charset=utf-8", b"Source path is missing");
            };
            let Some(path) = safe_project_file(project_root, Path::new(source_path)) else {
                return response(
                    403,
                    "text/plain; charset=utf-8",
                    b"Source path is not allowed",
                );
            };
            match fs::read(&path) {
                Ok(source) => response(200, "text/plain; charset=utf-8", &source),
                Err(error) => json_response(404, &json!({ "error": error.to_string() })),
            }
        }
        [_] => json_response(200, &metadata),
        _ => response(404, "text/plain; charset=utf-8", b"Unknown level resource"),
    }
}

fn level_file(id: &str) -> Option<&'static str> {
    match id {
        "level-001" => Some(LEVEL_FILES[0]),
        "level-002" => Some(LEVEL_FILES[1]),
        "level-003" => Some(LEVEL_FILES[2]),
        "level-004" => Some(LEVEL_FILES[3]),
        "level-005" => Some(LEVEL_FILES[4]),
        "level-006" => Some(LEVEL_FILES[5]),
        _ => None,
    }
}

fn read_json(path: PathBuf) -> Result<Value, serde_json::Error> {
    let text = fs::read_to_string(path).map_err(serde_json::Error::io)?;
    serde_json::from_str(&text)
}

fn safe_project_file(project_root: &Path, relative: &Path) -> Option<PathBuf> {
    if relative.is_absolute()
        || relative.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    let root = project_root.canonicalize().ok()?;
    let path = project_root.join(relative).canonicalize().ok()?;
    path.starts_with(root).then_some(path)
}

fn reading_response(project_root: &Path, relative: &str) -> Vec<u8> {
    let Some(path) = safe_reading_file(project_root, Path::new(relative)) else {
        return response(404, "text/plain; charset=utf-8", b"Reading not found");
    };
    match fs::read(&path) {
        Ok(contents) => response(200, content_type(&path), &contents),
        Err(_) => response(404, "text/plain; charset=utf-8", b"Reading not found"),
    }
}

fn safe_reading_file(project_root: &Path, relative: &Path) -> Option<PathBuf> {
    if relative.is_absolute()
        || relative.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }

    let rust_root = project_root.parent()?;
    let reading_root = rust_root.join("reading");
    let canonical_path = reading_root.join(relative).canonicalize().ok()?;
    let canonical_reading_root = reading_root.canonicalize().ok()?;
    if canonical_path.starts_with(&canonical_reading_root) {
        return Some(canonical_path);
    }

    // The offline-book links intentionally point into the installed rustup docs.
    [
        "offline-book",
        "offline-reference",
        "offline-rust-by-example",
        "offline-std",
    ]
    .iter()
    .filter_map(|folder| reading_root.join(folder).canonicalize().ok())
    .any(|allowed_root| canonical_path.starts_with(allowed_root))
    .then_some(canonical_path)
}

fn review_response(project_root: &Path) -> Vec<u8> {
    let commands: [(&str, &[&str]); 3] = [
        ("Formatting", &["fmt", "--all", "--", "--check"]),
        ("Compile check", &["check"]),
        (
            "Clippy",
            &[
                "clippy",
                "--bin",
                "ringen_van_patronage_origins",
                "--",
                "-D",
                "warnings",
            ],
        ),
    ];
    let checks = commands
        .iter()
        .map(|(label, arguments)| {
            let outcome = Command::new("cargo")
                .args(*arguments)
                .current_dir(project_root)
                .output();
            match outcome {
                Ok(output) => {
                    let details = format!(
                        "{}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                    json!({
                        "name": label,
                        "passed": output.status.success(),
                        "output": truncate(&details, 5000),
                    })
                }
                Err(error) => json!({
                    "name": label,
                    "passed": false,
                    "output": format!("Could not start cargo: {error}"),
                }),
            }
        })
        .collect::<Vec<_>>();
    let passed = checks.iter().all(|check| check["passed"] == true);
    json_response(200, &json!({ "passed": passed, "checks": checks }))
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        format!(
            "{}\n… output shortened",
            text.chars().take(max_chars).collect::<String>()
        )
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("md") => "text/markdown; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("wasm") => "application/wasm",
        Some("xml") => "application/xml",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn json_response(status: u16, value: &Value) -> Vec<u8> {
    match serde_json::to_vec(value) {
        Ok(body) => response(status, "application/json; charset=utf-8", &body),
        Err(error) => response(
            500,
            "text/plain; charset=utf-8",
            error.to_string().as_bytes(),
        ),
    }
}

fn response(status: u16, content_type: &str, body: &[u8]) -> Vec<u8> {
    let phrase = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        _ => "Internal Server Error",
    };
    let header = format!(
        "HTTP/1.1 {status} {phrase}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut response = header.into_bytes();
    response.extend_from_slice(body);
    response
}
