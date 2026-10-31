const state = {
  levels: [],
  level: null,
  source: "",
  dirty: false,
  buildRunning: false,
  tab: "learn",
  review: null,
  search: "",
};

// "live" talks to the local Rust server; "static" is the hosted copy, which reads JSON files
// exported by scripts/export-studio.sh and cannot run cargo.
const STATIC_MODE = document.querySelector('meta[name="studio-mode"]')?.content === "static";
const endpoints = {
  levels: STATIC_MODE ? "api/levels.json" : "api/levels",
  source: (levelId) => (STATIC_MODE ? `api/levels/${levelId}/source.txt` : `api/levels/${levelId}/source`),
};
const LOCAL_ONLY_HINT = "Available in the local studio: ./scripts/studio.sh";

const levelList = document.querySelector("#level-list");
const lessonContent = document.querySelector("#lesson-content");
const sourceEditor = document.querySelector("#source-editor");
const saveButton = document.querySelector("#save-recompile");
const gameFrame = document.querySelector("#game-frame");
const gamePanel = document.querySelector(".game-panel");

async function loadLevels() {
  try {
    const response = await fetch(endpoints.levels);
    if (!response.ok) throw new Error(`Server returned ${response.status}`);
    state.levels = await response.json();
    renderSidebar();
    const requestedId = location.hash.slice(1);
    const initial = state.levels.find((level) => level.id === requestedId) || state.levels[0];
    if (initial) await selectLevel(initial.id, false);
    else showFatal("No level data was found in data/levels.");
    initializeGamePreview();
  } catch (error) {
    showFatal(`Could not load lessons from ${STATIC_MODE ? "this site" : "the local Rust project"}. ${error.message}`);
  }
}

async function selectLevel(levelId, updateHash = true) {
  if (state.dirty && !confirm("Discard the unsaved edits in this source file?")) return;
  const level = state.levels.find((item) => item.id === levelId);
  if (!level) return;
  state.level = level;
  state.source = "";
  state.dirty = false;
  state.review = null;
  state.tab = "learn";
  if (updateHash) history.replaceState(null, "", `#${level.id}`);
  renderSidebar();
  renderHeading();
  renderTabs();
  renderTabContent();
  sourceEditor.value = "Fetching source…";
  sourceEditor.disabled = true;
  updateSourceControls();

  try {
    const response = await fetch(endpoints.source(level.id));
    if (!response.ok) throw new Error(`Source request returned ${response.status}`);
    state.source = await response.text();
    sourceEditor.disabled = false;
    renderSource();
  } catch (error) {
    sourceEditor.value = `Could not read ${level.source_path}: ${error.message}`;
    sourceEditor.classList.add("source-error");
  }
}

function renderSidebar() {
  levelList.replaceChildren();
  for (const level of state.levels) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `level-card${state.level?.id === level.id ? " active" : ""}`;
    button.setAttribute("aria-current", state.level?.id === level.id ? "page" : "false");

    const number = document.createElement("span");
    number.className = "level-card-number";
    number.textContent = `LEVEL ${String(level.number).padStart(2, "0")}`;
    const title = document.createElement("span");
    title.className = "level-card-title";
    title.textContent = level.title;
    const subtitle = document.createElement("span");
    subtitle.className = "level-card-subtitle";
    subtitle.textContent = level.subtitle;
    button.append(number, title, subtitle);
    button.addEventListener("click", () => selectLevel(level.id));
    levelList.append(button);
  }
}

function renderHeading() {
  const level = state.level;
  if (!level) return;
  document.querySelector("#level-kicker").textContent = `LEVEL ${String(level.number).padStart(2, "0")} · ${level.subtitle}`;
  document.querySelector("#level-title").textContent = level.title;
  document.querySelector("#level-description").textContent = level.description;
  document.querySelector("#level-algorithm").textContent = level.algorithm;
  document.querySelector("#source-path").textContent = level.source_path;
  document.querySelector("#study-level-label").textContent = `LEVEL ${String(level.number).padStart(2, "0")}`;
}

function renderTabs() {
  for (const tab of document.querySelectorAll(".tab")) {
    const active = tab.dataset.tab === state.tab;
    tab.classList.toggle("active", active);
    tab.setAttribute("aria-selected", String(active));
  }
}

function renderTabContent() {
  if (!state.level) return;
  renderTabs();
  lessonContent.replaceChildren();
  if (state.tab === "learn") renderLearn();
  if (state.tab === "review") renderReview();
  if (state.tab === "practice") renderPractice();
  if (state.tab === "readings") renderReadings();
}

function renderLearn() {
  const level = state.level;
  lessonContent.append(heading("Learning goals"), list(level.learning_goals));
  lessonContent.append(heading("What the code does"), paragraph(level.explanation));
  const algorithm = document.createElement("div");
  algorithm.className = "callout algorithm-callout";
  algorithm.textContent = level.algorithm;
  lessonContent.append(algorithm);
}

function renderReview() {
  lessonContent.append(heading("Review this level"), list(state.level.review_points));
  const note = paragraph("Read the source, predict what the systems change, and compare your explanation with the game.");
  lessonContent.append(note);

  const button = document.createElement("button");
  button.type = "button";
  button.className = "review-button";
  button.textContent = state.review?.running ? "Checks are running…" : "▶  Run project checks";
  button.disabled = STATIC_MODE || Boolean(state.review?.running);
  button.addEventListener("click", runProjectChecks);
  lessonContent.append(button);
  lessonContent.append(
    paragraph(
      STATIC_MODE
        ? `Runs rustfmt, cargo check, and Clippy. ${LOCAL_ONLY_HINT}. The same checks run on every push in CI.`
        : "Runs rustfmt, cargo check, and Clippy against the local game project.",
    ),
  );

  if (state.review?.error) {
    const error = document.createElement("div");
    error.className = "callout";
    error.textContent = state.review.error;
    lessonContent.append(error);
  }
  if (state.review?.checks) {
    const results = document.createElement("div");
    results.className = "checks-list";
    for (const check of state.review.checks) {
      const card = document.createElement("section");
      card.className = "check-card";
      const summary = document.createElement("div");
      summary.className = "check-summary";
      const name = document.createElement("span");
      name.textContent = check.name;
      const status = document.createElement("strong");
      status.className = check.passed ? "pass" : "fail";
      status.textContent = check.passed ? "PASS" : "NEEDS ATTENTION";
      summary.append(name, status);
      card.append(summary);
      if (check.output) {
        const output = document.createElement("pre");
        output.className = "check-output";
        output.textContent = check.output;
        card.append(output);
      }
      results.append(card);
    }
    lessonContent.append(results);
  }
}

async function runProjectChecks() {
  if (STATIC_MODE) return;
  state.review = { running: true };
  renderTabContent();
  try {
    const response = await fetch("api/review", { method: "POST" });
    if (!response.ok) throw new Error(`Review returned ${response.status}`);
    state.review = await response.json();
  } catch (error) {
    state.review = { error: `Could not finish the local review. ${error.message}` };
  }
  renderTabContent();
}

function renderPractice() {
  lessonContent.append(heading("Try it yourself"), list(state.level.practice_tasks));
  const tip = document.createElement("div");
  tip.className = "callout";
  tip.textContent = "Change one thing at a time. Save a working version, predict the result, then run the game and compare.";
  lessonContent.append(tip);
}

function renderReadings() {
  lessonContent.append(heading("Read next"));
  const listElement = document.createElement("div");
  listElement.className = "reading-list";
  for (const reading of state.level.readings || []) {
    if (STATIC_MODE && !reading.url) continue;
    const card = document.createElement("a");
    card.className = "reading-card";
    card.target = "_blank";
    card.rel = "noreferrer";
    const useLocalEdition = !STATIC_MODE && reading.local_path?.startsWith("reading/");
    if (useLocalEdition) {
      card.href = `/reading/${reading.local_path.slice("reading/".length)}`;
    } else {
      card.href = reading.url;
    }
    const title = document.createElement("strong");
    title.textContent = reading.title;
    const description = document.createElement("span");
    description.textContent = useLocalEdition ? "Open local offline edition ↗" : "Open official online reference ↗";
    card.append(title, description);
    listElement.append(card);

    if (useLocalEdition && reading.url) {
      const online = document.createElement("a");
      online.className = "reading-card online-reading";
      online.href = reading.url;
      online.target = "_blank";
      online.rel = "noreferrer";
      const onlineTitle = document.createElement("strong");
      onlineTitle.textContent = `${reading.title} · online`;
      const onlineDescription = document.createElement("span");
      onlineDescription.textContent = "Open official website ↗";
      online.append(onlineTitle, onlineDescription);
      listElement.append(online);
    }
  }
  lessonContent.append(listElement);
}

function renderSource() {
  sourceEditor.classList.remove("source-error");
  sourceEditor.value = state.source;
  const lines = state.source.split("\n");
  document.querySelector("#source-line-count").textContent = `${lines.length} LINES`;
  updateSourceControls();
}

function updateSourceControls() {
  const status = document.querySelector("#source-state");
  status.textContent = STATIC_MODE ? "READ-ONLY SOURCE" : state.dirty ? "UNSAVED EDITS" : "SAVED SOURCE";
  status.classList.toggle("dirty", state.dirty);
  saveButton.disabled = STATIC_MODE || !state.dirty || state.buildRunning || sourceEditor.disabled;
  document.querySelector("#source-line-count").textContent = `${sourceEditor.value.split("\n").length} LINES`;
}

function setBuildStatus(message, tone = "") {
  const status = document.querySelector("#game-build-status");
  status.textContent = message;
  status.classList.toggle("error", tone === "error");
  status.classList.toggle("busy", tone === "busy");
}

function showBuildOutput(output) {
  const details = document.querySelector("#build-details");
  details.hidden = !output;
  document.querySelector("#build-output").textContent = output;
}

async function initializeGamePreview() {
  if (STATIC_MODE) {
    setBuildStatus("Ready · prebuilt Rust game running in this page");
    return;
  }
  try {
    const response = await fetch("api/game/build");
    if (!response.ok) throw new Error(`Build status returned ${response.status}`);
    const status = await response.json();
    if (status.available && !status.running) {
      setBuildStatus("Ready · Rust game running in this page");
      showBuildOutput(status.output);
      return;
    }
    if (status.running) {
      await startBrowserBuild();
      return;
    }
    await startBrowserBuild();
  } catch (error) {
    setBuildStatus(`Game build unavailable: ${error.message}`, "error");
  }
}

async function startBrowserBuild() {
  if (STATIC_MODE) return;
  if (state.dirty) {
    setBuildStatus("Save the edited source before rebuilding.", "error");
    return;
  }
  state.buildRunning = true;
  updateSourceControls();
  document.querySelector("#rebuild-game").disabled = true;
  setBuildStatus("Compiling Rust for the browser…", "busy");
  showBuildOutput("");
  try {
    const response = await fetch("api/game/build", { method: "POST" });
    if (!response.ok && response.status !== 409) {
      const body = await response.text();
      throw new Error(body || `Build request returned ${response.status}`);
    }
    await pollBuild();
  } catch (error) {
    setBuildStatus(`Build could not start: ${error.message}`, "error");
  } finally {
    state.buildRunning = false;
    updateSourceControls();
    document.querySelector("#rebuild-game").disabled = false;
  }
}

async function pollBuild() {
  while (true) {
    await new Promise((resolve) => setTimeout(resolve, 900));
    const response = await fetch("api/game/build");
    if (!response.ok) throw new Error(`Build status returned ${response.status}`);
    const status = await response.json();
    if (status.running) {
      setBuildStatus("Compiling Rust for the browser…", "busy");
      continue;
    }
    showBuildOutput(status.output);
    if (status.success) {
      setBuildStatus("Ready · Rust game running in this page");
      reloadGame();
    } else {
      setBuildStatus("Build failed · see build output", "error");
    }
    return;
  }
}

async function saveAndRecompile() {
  if (STATIC_MODE) return;
  if (!state.level || !state.dirty || state.buildRunning) return;
  saveButton.disabled = true;
  saveButton.textContent = "Saving…";
  setBuildStatus(`Saving ${state.level.source_path}…`, "busy");
  const source = sourceEditor.value;
  try {
    const response = await fetch(`api/levels/${state.level.id}/source`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ source }),
    });
    if (!response.ok) throw new Error((await response.text()) || `Save returned ${response.status}`);
    state.source = source;
    state.dirty = false;
    renderSource();
    saveButton.textContent = "Save & Recompile";
    await startBrowserBuild();
  } catch (error) {
    setBuildStatus(`Could not save source: ${error.message}`, "error");
    saveButton.disabled = false;
    saveButton.textContent = "Save & Recompile";
  }
}

function reloadGame() {
  gameFrame.src = `game/?reload=${Date.now()}`;
}

function list(items = []) {
  const ul = document.createElement("ul");
  for (const item of items) {
    const li = document.createElement("li");
    li.textContent = item;
    ul.append(li);
  }
  return ul;
}

function heading(text) {
  const element = document.createElement("h2");
  element.textContent = text;
  return element;
}

function paragraph(text) {
  const element = document.createElement("p");
  element.textContent = text;
  return element;
}

function showFatal(message) {
  lessonContent.replaceChildren(heading("Viewer could not load"), paragraph(message));
  levelList.textContent = STATIC_MODE ? "The lesson data failed to load." : "Check that the game project is available.";
  sourceEditor.value = message;
  sourceEditor.classList.add("source-error");
}

document.querySelectorAll(".tab").forEach((tab) => {
  tab.addEventListener("click", () => {
    state.tab = tab.dataset.tab;
    renderTabContent();
  });
});

document.querySelector("#source-search").addEventListener("input", (event) => {
  state.search = event.target.value;
});

document.querySelector("#source-search").addEventListener("keydown", (event) => {
  if (event.key !== "Enter" || !event.currentTarget.value) return;
  const query = event.currentTarget.value.toLowerCase();
  const source = sourceEditor.value.toLowerCase();
  const from = sourceEditor.selectionEnd;
  const found = source.indexOf(query, from);
  const start = found >= 0 ? found : source.indexOf(query);
  if (start >= 0) {
    sourceEditor.focus();
    sourceEditor.setSelectionRange(start, start + query.length);
  }
});

document.querySelector("#refresh-source").addEventListener("click", () => {
  if (state.level) selectLevel(state.level.id, false);
});

document.querySelector("#copy-source").addEventListener("click", async (event) => {
  if (!sourceEditor.value) return;
  const button = event.currentTarget;
  try {
    await navigator.clipboard.writeText(sourceEditor.value);
    button.textContent = "Copied";
    setTimeout(() => { button.textContent = "Copy"; }, 1200);
  } catch {
    button.textContent = "Copy blocked";
  }
});

sourceEditor.addEventListener("input", () => {
  state.dirty = sourceEditor.value !== state.source;
  updateSourceControls();
});

saveButton.addEventListener("click", saveAndRecompile);
document.querySelector("#rebuild-game").addEventListener("click", startBrowserBuild);
document.querySelector("#reload-game").addEventListener("click", reloadGame);
document.querySelector("#expand-game").addEventListener("click", (event) => {
  const expanded = gamePanel.classList.toggle("expanded");
  event.currentTarget.setAttribute("aria-expanded", String(expanded));
  event.currentTarget.textContent = expanded ? "Shrink game" : "Expand game";
});

function applyStaticMode() {
  document.querySelector("#static-notice").hidden = false;
  document.querySelector("#workspace-label").textContent = "HOSTED COPY";
  document.querySelector("#footer-note").textContent = "Source is read-only here; run the studio locally to edit and recompile.";
  document.querySelector("#footer-scope").textContent = "Hosted copy · no account or upload";
  document.querySelector("#reference-link-text").textContent = "Rust documentation";
  document.querySelector("#reference-link").href = "https://doc.rust-lang.org/";
  document.querySelector(".sidebar-bottom p").textContent = "Rust Book, Rust by Example, standard library, and language reference (online).";
  sourceEditor.readOnly = true;
  sourceEditor.setAttribute("aria-label", "Rust source code (read-only)");
  saveButton.textContent = "Edit locally";
  saveButton.title = LOCAL_ONLY_HINT;
  const rebuild = document.querySelector("#rebuild-game");
  rebuild.hidden = true;
}

if (STATIC_MODE) applyStaticMode();
loadLevels();
