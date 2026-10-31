// Smoke test for the Learning Studio page script (viewer/app.js) against a stubbed DOM.
// Runs under Node, or under JavaScriptCore's `jsc` shell where Node is not installed.
//
//   node scripts/studio-smoke.js <static|live> <exported-site-dir> <path-to-app.js> [#level-id]
//
// It checks that each mode requests only the files that mode can serve, never uses root-absolute
// URLs (which break under a GitHub Pages sub-path), and that the hosted copy switches off every
// action that needs cargo.
const isNode = typeof process !== "undefined" && Boolean(process.versions && process.versions.node);
const readText = isNode ? (path) => require("fs").readFileSync(path, "utf8") : (path) => read(path);
const output = isNode ? (line) => console.log(line) : (line) => print(line);
const cliArgs = isNode ? process.argv.slice(2) : Array.from(arguments);
const [mode, siteDir, appPath, levelHash = "#level-006"] = cliArgs;
if (!["static", "live"].includes(mode) || !siteDir || !appPath) {
  output("usage: studio-smoke.js <static|live> <exported-site-dir> <path-to-app.js> [#level-id]");
  if (isNode) process.exit(2);
  else quit(2);
}

const requests = [];
const elements = {};
function makeElement(selector) {
  return {
    selector, children: [], style: {}, dataset: {}, attrs: {}, listeners: {}, textContent: "", value: "",
    href: "", title: "", className: "", src: "", selectionEnd: 0, disabled: false, readOnly: false,
    hidden: selector === "#static-notice",
    classList: { add() {}, remove() {}, toggle() {}, contains() { return false; } },
    append(...nodes) { this.children.push(...nodes); },
    replaceChildren(...nodes) { this.children = nodes; },
    setAttribute(name, value) { this.attrs[name] = value; },
    addEventListener(type, listener) { this.listeners[type] = listener; },
    focus() {}, setSelectionRange() {},
  };
}
const tabs = ["learn", "review", "practice", "readings"].map((name) => {
  const tab = makeElement(".tab");
  tab.dataset.tab = name;
  return tab;
});

globalThis.document = {
  querySelector(selector) {
    if (selector === 'meta[name="studio-mode"]') return { content: mode };
    return (elements[selector] ||= makeElement(selector));
  },
  querySelectorAll(selector) { return selector === ".tab" ? tabs : []; },
  createElement(tag) { return makeElement(tag); },
};
globalThis.location = { hash: levelHash };
globalThis.history = { replaceState() {} };
globalThis.confirm = () => true;
globalThis.navigator = { clipboard: { writeText: async () => {} } };
globalThis.setTimeout = (callback) => callback;

// Serves the same payloads the local Rust server and the exported site provide.
globalThis.fetch = (url, options = {}) => {
  requests.push(`${options.method || "GET"} ${url}`);
  let body = null;
  if (url === "api/levels.json" || url === "api/levels") {
    body = readText(`${siteDir}/api/levels.json`);
  } else if (/^api\/levels\/level-\d+\/source(\.txt)?$/.test(url)) {
    body = readText(`${siteDir}/${url.replace(/\/source$/, "/source.txt")}`);
  } else if (url === "api/game/build") {
    body = JSON.stringify({ available: true, running: false, success: true, output: "" });
  }
  if (body === null) {
    return Promise.resolve({ ok: false, status: 404, text: async () => "", json: async () => ({}) });
  }
  return Promise.resolve({ ok: true, status: 200, text: async () => body, json: async () => JSON.parse(body) });
};

const failures = [];
function check(name, passed, detail = "") {
  output(`${passed ? "PASS" : "FAIL"}  ${name}${detail ? `  -> ${detail}` : ""}`);
  if (!passed) failures.push(name);
}
const E = (selector) => document.querySelector(selector);
function collect(node, pick, found = []) {
  const value = pick(node);
  if (value) found.push(value);
  (node.children || []).forEach((child) => collect(child, pick, found));
  return found;
}
async function settle() {
  for (let turn = 0; turn < 60; turn += 1) await Promise.resolve();
}

async function main() {
  const source = readText(appPath);
  if (isNode) require("vm").runInThisContext(source, { filename: appPath });
  else load(appPath);
  await settle();

  check("sidebar lists every level", E("#level-list").children.length === 6, String(E("#level-list").children.length));
  check("selected level title is shown", E("#level-title").textContent === "The Ringways", E("#level-title").textContent);
  check("source loaded", E("#source-editor").value.startsWith("use super::"), E("#source-editor").value.slice(0, 20));
  check("lesson renders", collect(E("#lesson-content"), (n) => n.textContent).join(" ").includes("Learning goals"));

  tabs[1].listeners.click();
  const reviewButton = E("#lesson-content").children.find((child) => child.className === "review-button");
  check("review tab renders", Boolean(reviewButton));
  check(`review button is ${mode === "static" ? "disabled" : "enabled"}`, Boolean(reviewButton) && reviewButton.disabled === (mode === "static"));

  tabs[3].listeners.click();
  const links = collect(E("#lesson-content"), (n) => n.href);
  if (mode === "static") {
    check("readings are online links only", links.length > 0 && links.every((href) => href.startsWith("https://")), links.join(", "));
  } else {
    check("readings include local editions", links.some((href) => href.startsWith("/reading/")));
  }

  if (mode === "static") {
    check("hosted-copy notice is visible", E("#static-notice").hidden === false);
    check("source is read-only", E("#source-editor").readOnly === true);
    check("source label says read-only", E("#source-state").textContent === "READ-ONLY SOURCE", E("#source-state").textContent);
    check("save & recompile is disabled", E("#save-recompile").disabled === true);
    check("recompile game is hidden", E("#rebuild-game").hidden === true);
    check("game shows as ready", E("#game-build-status").textContent.startsWith("Ready"), E("#game-build-status").textContent);
    check("docs link points online", E("#reference-link").href === "https://doc.rust-lang.org/");
    E("#save-recompile").listeners.click();
    E("#rebuild-game").listeners.click();
    if (reviewButton.listeners.click) reviewButton.listeners.click();
    await settle();
    const writes = requests.filter((request) => !request.startsWith("GET ") || request.includes("api/game/build") || request.includes("api/review"));
    check("hosted copy never writes, builds, or reviews", writes.length === 0, writes.join(" | "));
  } else {
    check("local mode hides the hosted-copy notice", E("#static-notice").hidden === true);
    check("source is editable", E("#source-editor").readOnly === false);
    check("local mode asks the server for the build status", requests.includes("GET api/game/build"));
  }

  const absolute = requests.filter((request) => /\s\//.test(request));
  check("no root-absolute requests", absolute.length === 0, absolute.join(", "));
  output(`requests: ${requests.join(" | ")}`);
  output(failures.length ? `FAILED: ${failures.length}` : "ALL PASSED");
  if (isNode) process.exitCode = failures.length ? 1 : 0;
  else quit(failures.length ? 1 : 0);
}

main().catch((error) => {
  output(`FAIL  smoke test crashed: ${error && error.stack ? error.stack : error}`);
  if (isNode) process.exitCode = 1;
  else quit(1);
});
