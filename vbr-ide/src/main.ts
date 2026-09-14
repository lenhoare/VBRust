import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { registerVbrLanguage, VBR_LANGUAGE_ID } from "./vbrLanguage";
import { EXAMPLES } from "./examples";

// Monaco needs a worker for the editor itself; Vinyl and Rust are both
// Monarch-tokenised on the main thread here, so the base editor worker is all
// we wire up. (Real Vinyl tokenisation lands in slice 6.)
self.MonacoEnvironment = {
  getWorker: () => new editorWorker(),
};

interface Range {
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
}

interface Diagnostic {
  level: "error" | "warning" | "note";
  message: string;
  line: number | null;
  range: Range | null;
}

interface TranspileResult {
  code: string;
  language: string;
  diagnostics: Diagnostic[];
  /** `[generatedLine, bustLine]` 1-based checkpoints (Tide's line map). */
  line_map: [number, number][];
}

interface RunOutput {
  stage: "diagnostics" | "compile" | "run" | "project";
  rust: string;
  diagnostics: Diagnostic[];
  stdout: string;
  stderr: string;
  success: boolean;
}

interface OpenedFile {
  path: string;
  content: string;
}

interface CompletionItem {
  label: string;
  detail: string;
  kind: string;
}

interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  children: FileEntry[];
}

interface Project {
  root: string;
  name: string;
  is_project: boolean;
  entry: string | null;
  files: FileEntry[];
}

const SAMPLE = `' Welcome to Vinyl — type on the left, read the Rust on the right.
Function Main()
    Dim name As String = "world"
    Debug.Print "Hello, " & name & "!"

    Dim total As Long = 0
    For i = 1 To 10
        total = total + i
    Next i
    Debug.Print "Sum 1..10 = " & total
End Function
`;

// Register the 'vbr' language and its Monarch tokeniser (keywords, strings,
// comments, numbers, and the verbatim Rust/Python/Text blocks).
registerVbrLanguage(monaco);

// Completion + hover, served in-process by the compiler (no LSP server needed —
// the intelligence is a library call away).
function monacoKind(kind: string): monaco.languages.CompletionItemKind {
  const K = monaco.languages.CompletionItemKind;
  switch (kind) {
    case "method": return K.Method;
    case "field": return K.Field;
    case "variable": return K.Variable;
    case "function": return K.Function;
    case "constant": return K.Constant;
    case "namespace": return K.Module;
    case "enumvariant": return K.EnumMember;
    case "enum": return K.Enum;
    case "struct": return K.Struct;
    case "keyword": return K.Keyword;
    default: return K.Text;
  }
}

monaco.languages.registerCompletionItemProvider(VBR_LANGUAGE_ID, {
  triggerCharacters: ["."],
  async provideCompletionItems(model, position) {
    const items = await invoke<CompletionItem[]>("complete_at", {
      source: model.getValue(),
      line: position.lineNumber,
      col: position.column,
    });
    const word = model.getWordUntilPosition(position);
    const range = new monaco.Range(
      position.lineNumber,
      word.startColumn,
      position.lineNumber,
      word.endColumn,
    );
    return {
      suggestions: items.map((it) => ({
        label: it.label,
        detail: it.detail,
        kind: monacoKind(it.kind),
        insertText: it.label,
        range,
      })),
    };
  },
});

monaco.languages.registerHoverProvider(VBR_LANGUAGE_ID, {
  async provideHover(model, position) {
    const text = await invoke<string | null>("hover_at", {
      source: model.getValue(),
      line: position.lineNumber,
      col: position.column,
    });
    return text ? { contents: [{ value: text }] } : null;
  },
});

monaco.languages.registerDefinitionProvider(VBR_LANGUAGE_ID, {
  async provideDefinition(model, position) {
    const r = await invoke<Range | null>("definition_at", {
      source: model.getValue(),
      line: position.lineNumber,
      col: position.column,
    });
    return r ? { uri: model.uri, range: r } : null;
  },
});

// Start blank each launch (not the last-open file). SAMPLE is available via the
// examples picker instead.
void SAMPLE;

const ACCENT_KEY = "vbr-ide.accent";
const ACCENT_DEFAULT = "#5ec8c5";

function readAccent(): string {
  const stored = localStorage.getItem(ACCENT_KEY);
  return stored && /^#[0-9a-fA-F]{6}$/.test(stored) ? stored : ACCENT_DEFAULT;
}

function applyAccent(hex: string): void {
  const color = /^#[0-9a-fA-F]{6}$/.test(hex) ? hex : ACCENT_DEFAULT;
  document.documentElement.style.setProperty("--accent", color);
  localStorage.setItem(ACCENT_KEY, color);
  const swatch = document.getElementById("accent") as HTMLInputElement | null;
  if (swatch) swatch.value = color;
  defineMonacoThemes();
}

function defineMonacoThemes(): void {
  const accent =
    getComputedStyle(document.documentElement).getPropertyValue("--accent").trim() ||
    ACCENT_DEFAULT;
  monaco.editor.defineTheme("bust-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#1e1f22",
      "editor.lineHighlightBackground": `${accent}22`,
      "editor.selectionBackground": `${accent}44`,
      "editorCursor.foreground": accent,
      "editorLineNumber.activeForeground": accent,
    },
  });
  monaco.editor.defineTheme("bust-light", {
    base: "vs",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#ffffff",
      "editor.lineHighlightBackground": `${accent}18`,
      "editor.selectionBackground": `${accent}33`,
      "editorCursor.foreground": accent,
      "editorLineNumber.activeForeground": accent,
    },
  });
  monaco.editor.setTheme(document.body.classList.contains("light") ? "bust-light" : "bust-dark");
}

applyAccent(readAccent());
(document.getElementById("accent") as HTMLInputElement).addEventListener("input", (e) => {
  applyAccent((e.target as HTMLInputElement).value);
});

const editor = monaco.editor.create(document.getElementById("editor")!, {
  value: "",
  language: VBR_LANGUAGE_ID,
  theme: "bust-dark",
  minimap: { enabled: false },
  fontSize: 14,
  automaticLayout: true,
  scrollBeyondLastLine: false,
  mouseWheelZoom: true,
});

const rustView = monaco.editor.create(document.getElementById("rust")!, {
  value: "",
  language: "rust",
  theme: "bust-dark",
  readOnly: true,
  minimap: { enabled: false },
  fontSize: 14,
  automaticLayout: true,
  scrollBeyondLastLine: false,
  mouseWheelZoom: true,
});

const diagnosticsEl = document.getElementById("diagnostics")!;

// --- Tabs (one Monaco model per open file) ---------------------------------

interface Tab {
  id: number;
  path: string | null; // null = an untitled scratch buffer
  model: monaco.editor.ITextModel;
  dirty: boolean;
  isProjectFile: boolean;
}
const tabs: Tab[] = [];
let activeId = -1;
let tabSeq = 1;
const tabbarEl = document.getElementById("tabbar")!;

function activeTab(): Tab | undefined {
  return tabs.find((t) => t.id === activeId);
}
function basename(p: string): string {
  return p.split(/[/\\]/).pop() ?? p;
}
function langForPath(path: string | null): string {
  if (!path) return VBR_LANGUAGE_ID;
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    vbr: VBR_LANGUAGE_ID, rs: "rust", toml: "ini", json: "json", md: "markdown",
    yaml: "yaml", yml: "yaml", html: "html", css: "css", js: "javascript",
    ts: "typescript", py: "python", xml: "xml", sh: "shell", ini: "ini", cfg: "ini",
  };
  return map[ext] ?? "plaintext";
}
// An untitled buffer or a .vbr file gets the Rust pane; anything else blanks it.
function isVbrTab(t: Tab | undefined): boolean {
  return !!t && (t.path === null || t.path.toLowerCase().endsWith(".vbr"));
}

function makeTab(path: string | null, content: string, isProjectFile: boolean): Tab {
  const model = monaco.editor.createModel(content, langForPath(path));
  const tab: Tab = { id: tabSeq++, path, model, dirty: false, isProjectFile };
  model.onDidChangeContent(() => {
    if (!tab.dirty) {
      tab.dirty = true;
      renderTabs();
      if (tab.id === activeId) updateFilename();
    }
  });
  tabs.push(tab);
  return tab;
}

function activateTab(id: number): void {
  const tab = tabs.find((t) => t.id === id);
  if (!tab) return;
  activeId = id;
  editor.setModel(tab.model);
  currentPath = tab.path;
  isProject = tab.isProjectFile;
  renderTabs();
  updateFilename();
  updateProjectButtons();
  void refresh();
}

// Open a file in a tab: reuse an existing tab for the same path, else make one.
function openTab(path: string | null, content: string, isProjectFile = false): void {
  if (path) {
    const existing = tabs.find((t) => t.path === path);
    if (existing) {
      activateTab(existing.id);
      return;
    }
  }
  activateTab(makeTab(path, content, isProjectFile).id);
}

function closeTab(id: number): void {
  const tab = tabs.find((t) => t.id === id);
  if (!tab) return;
  if (
    tab.dirty &&
    !window.confirm(`${tab.path ? basename(tab.path) : "untitled"} has unsaved changes. Close without saving?`)
  ) {
    return;
  }
  const idx = tabs.indexOf(tab);
  tabs.splice(idx, 1);
  tab.model.dispose();
  if (tabs.length === 0) {
    openTab(null, ""); // always keep at least one tab
    return;
  }
  if (activeId === id) activateTab(tabs[Math.min(idx, tabs.length - 1)].id);
  else renderTabs();
}

function renderTabs(): void {
  tabbarEl.innerHTML = "";
  for (const tab of tabs) {
    const el = document.createElement("div");
    el.className = "tab" + (tab.id === activeId ? " active" : "");
    const name = document.createElement("span");
    name.textContent = tab.path ? basename(tab.path) : "untitled";
    el.appendChild(name);
    if (tab.dirty) {
      const dot = document.createElement("span");
      dot.className = "tab-dot";
      dot.textContent = "●";
      el.appendChild(dot);
    }
    const close = document.createElement("span");
    close.className = "tab-close";
    close.textContent = "×";
    close.title = "Close";
    close.addEventListener("click", (e) => {
      e.stopPropagation();
      closeTab(tab.id);
    });
    el.appendChild(close);
    el.addEventListener("click", () => activateTab(tab.id));
    tabbarEl.appendChild(el);
  }
}

// The chosen output target (Rust / Python / C), persisted across sessions. Rust
// is primary; Python and C are additive views of the same source.
const TARGET_KEY = "vbr-ide.target";
const TARGET_LABELS: Record<string, string> = { rust: "Rust", python: "Python", c: "C" };
const targetBtns = [...document.querySelectorAll<HTMLButtonElement>(".target-btn")];
const generatedToolBtn = document.getElementById("tool-generated") as HTMLButtonElement;
const generatedIcons = [
  ...generatedToolBtn.querySelectorAll<SVGElement>("[data-target-icon]"),
];
let currentTarget = localStorage.getItem(TARGET_KEY) ?? "rust";

function applyTargetButtons(): void {
  for (const btn of targetBtns) {
    btn.classList.toggle("active", btn.dataset.target === currentTarget);
  }
  const label = TARGET_LABELS[currentTarget] ?? "Rust";
  generatedToolBtn.title = `Generated output (${label})`;
  for (const icon of generatedIcons) {
    icon.classList.toggle("is-current", icon.dataset.targetIcon === currentTarget);
  }
}

function setTarget(next: string): void {
  if (next === currentTarget) return;
  currentTarget = next;
  localStorage.setItem(TARGET_KEY, currentTarget);
  applyTargetButtons();
  void refresh();
}

for (const btn of targetBtns) {
  btn.addEventListener("click", () => setTarget(btn.dataset.target ?? "rust"));
}
applyTargetButtons();

// Same `(generated, bust)` checkpoints Tide uses. Empty → proportional scroll.
let lineMap: [number, number][] = [];
let syncingLines = false;

function rustSpanForVbr(
  map: [number, number][],
  vbr1: number,
  rustLines: number,
): { start: number; end: number } | null {
  if (!map.length || rustLines === 0 || vbr1 === 0) return null;
  let startR: number | undefined;
  let endR: number | undefined;
  for (let i = 0; i < map.length; i++) {
    const [r1, v] = map[i];
    if (v !== vbr1) continue;
    const next = map[i + 1];
    const segEnd = next ? Math.max(r1, next[0] - 1) : Math.max(r1, rustLines);
    startR = startR === undefined ? r1 : Math.min(startR, r1);
    endR = endR === undefined ? segEnd : Math.max(endR, segEnd);
  }
  if (startR !== undefined && endR !== undefined) {
    return { start: startR, end: Math.min(endR, rustLines) };
  }
  const earlier = [...map].reverse().find(([, v]) => v <= vbr1) ?? map[0];
  const r = Math.min(earlier[0], rustLines);
  return { start: r, end: r };
}

function vbrLineForRust(map: [number, number][], rust1: number): number | null {
  let last: number | null = null;
  for (const [r, v] of map) {
    if (r > rust1) break;
    last = v;
  }
  return last;
}

function syncRustFromVbr(): void {
  if (syncingLines) return;
  const vbrLine = editor.getPosition()?.lineNumber ?? 1;
  const rustLines = rustView.getModel()?.getLineCount() ?? 0;
  let start: number;
  if (lineMap.length) {
    const span = rustSpanForVbr(lineMap, vbrLine, rustLines);
    if (!span) return;
    start = span.start;
  } else if (rustLines > 0) {
    const vCount = editor.getModel()?.getLineCount() ?? 1;
    start = Math.min(rustLines, Math.max(1, Math.round((vbrLine / vCount) * rustLines)));
  } else {
    return;
  }
  if ((rustView.getPosition()?.lineNumber ?? 0) === start) return;
  syncingLines = true;
  rustView.setPosition({ lineNumber: start, column: 1 });
  rustView.revealLineInCenterIfOutsideViewport(start);
  syncingLines = false;
}

function syncVbrFromRust(): void {
  if (syncingLines) return;
  const rustLine = rustView.getPosition()?.lineNumber ?? 1;
  const rustLines = rustView.getModel()?.getLineCount() ?? 1;
  const vCount = editor.getModel()?.getLineCount() ?? 1;
  let vbr: number;
  if (lineMap.length) {
    const mapped = vbrLineForRust(lineMap, rustLine);
    if (mapped == null) return;
    vbr = mapped;
  } else {
    vbr = Math.min(vCount, Math.max(1, Math.round((rustLine / Math.max(1, rustLines)) * vCount)));
  }
  if ((editor.getPosition()?.lineNumber ?? 0) === vbr) return;
  syncingLines = true;
  const col = editor.getPosition()?.column ?? 1;
  const maxCol = editor.getModel()?.getLineMaxColumn(vbr) ?? 1;
  editor.setPosition({ lineNumber: vbr, column: Math.min(col, maxCol) });
  editor.revealLineInCenterIfOutsideViewport(vbr);
  syncingLines = false;
}

async function refresh(): Promise<void> {
  // Non-Vinyl files keep the split but blank the output view.
  if (!isVbrTab(activeTab())) {
    lineMap = [];
    rustView.setValue("");
    const m = editor.getModel();
    if (m) monaco.editor.setModelMarkers(m, "vbr", []);
    const path = activeTab()?.path;
    diagnosticsEl.innerHTML = `<span class="ok">— ${path ? escapeHtml(basename(path)) : "file"} is not a Vinyl file —</span>`;
    statusProblems.textContent = "";
    statusTiming.textContent = "";
    return;
  }
  const source = editor.getValue();
  try {
    const t0 = performance.now();
    const result = await invoke<TranspileResult>("transpile_source", {
      source,
      target: currentTarget,
    });
    const ms = Math.max(1, Math.round(performance.now() - t0));
    rustView.setValue(result.code);
    // Highlight the output pane in the target's own language.
    const outModel = rustView.getModel();
    if (outModel) monaco.editor.setModelLanguage(outModel, result.language);
    lineMap = result.line_map ?? [];
    renderDiagnostics(result.diagnostics);
    setMarkers(result.diagnostics);
    updateStatus(result.diagnostics, ms);
    syncRustFromVbr();
  } catch (e) {
    diagnosticsEl.textContent = String(e);
  }
}

const statusPos = document.getElementById("status-pos")!;
const statusProblems = document.getElementById("status-problems")!;
const statusTiming = document.getElementById("status-timing")!;

editor.onDidChangeCursorPosition((e) => {
  statusPos.textContent = `Ln ${e.position.lineNumber}, Col ${e.position.column}`;
  if (editor.hasTextFocus()) syncRustFromVbr();
});
rustView.onDidChangeCursorPosition(() => {
  if (rustView.hasTextFocus()) syncVbrFromRust();
});

function updateStatus(diags: Diagnostic[], ms: number): void {
  const errors = diags.filter((d) => d.level === "error").length;
  const warnings = diags.filter((d) => d.level === "warning").length;
  const parts: string[] = [];
  if (errors) parts.push(`${errors} error${errors === 1 ? "" : "s"}`);
  if (warnings) parts.push(`${warnings} warning${warnings === 1 ? "" : "s"}`);
  statusProblems.textContent = parts.length ? parts.join(", ") : "no problems";
  statusTiming.textContent = `updated in ${ms} ms`;
}

function severityOf(level: Diagnostic["level"]): monaco.MarkerSeverity {
  switch (level) {
    case "error":
      return monaco.MarkerSeverity.Error;
    case "warning":
      return monaco.MarkerSeverity.Warning;
    default:
      return monaco.MarkerSeverity.Info;
  }
}

// Paint the diagnostics as inline squiggles on the Vinyl pane. A diagnostic with
// a pinned span underlines exactly that span; a line-only one underlines its
// whole line; a diagnostic with neither (a top-level teaching note) shows only
// in the strip below.
function setMarkers(diags: Diagnostic[]): void {
  const model = editor.getModel();
  if (!model) return;
  const markers: monaco.editor.IMarkerData[] = [];
  for (const d of diags) {
    let range = d.range;
    if (!range && d.line && d.line <= model.getLineCount()) {
      range = {
        startLineNumber: d.line,
        startColumn: 1,
        endLineNumber: d.line,
        endColumn: model.getLineMaxColumn(d.line),
      };
    }
    if (!range) continue;
    markers.push({
      severity: severityOf(d.level),
      message: d.message,
      startLineNumber: range.startLineNumber,
      startColumn: range.startColumn,
      endLineNumber: range.endLineNumber,
      endColumn: range.endColumn,
    });
  }
  monaco.editor.setModelMarkers(model, "vbr", markers);
}

const DIAG_FILTER_KEY = "vbr-ide.diag-filter";
const diagFilterBtns = [...document.querySelectorAll<HTMLButtonElement>(".diag-filter")];
let showErrors = localStorage.getItem(`${DIAG_FILTER_KEY}.error`) !== "0";
let showWarnings = localStorage.getItem(`${DIAG_FILTER_KEY}.warning`) !== "0";
let lastDiags: Diagnostic[] = [];

function applyDiagFilters(): void {
  for (const btn of diagFilterBtns) {
    const on = btn.dataset.level === "error" ? showErrors : showWarnings;
    btn.classList.toggle("active", on);
  }
}

function renderDiagnostics(diags: Diagnostic[]): void {
  lastDiags = diags;
  const visible = diags.filter((d) => {
    if (d.level === "error") return showErrors;
    if (d.level === "warning") return showWarnings;
    return true;
  });
  if (visible.length === 0) {
    diagnosticsEl.innerHTML = diags.length
      ? `<span class="ok">— hidden by filters —</span>`
      : `<span class="ok">✓ no diagnostics</span>`;
    return;
  }
  diagnosticsEl.innerHTML = visible
    .map((d) => {
      const icon = d.level === "error" ? "✘" : d.level === "warning" ? "⚠" : "ℹ";
      const line = d.range?.startLineNumber ?? d.line ?? 0;
      const where = line ? `line ${line}: ` : "";
      const attr = line ? ` data-line="${line}"` : "";
      return `<div class="diag ${d.level}"${attr}>${icon} ${where}${escapeHtml(d.message)}</div>`;
    })
    .join("");
}

for (const btn of diagFilterBtns) {
  btn.addEventListener("click", () => {
    if (btn.dataset.level === "error") {
      showErrors = !showErrors;
      localStorage.setItem(`${DIAG_FILTER_KEY}.error`, showErrors ? "1" : "0");
    } else {
      showWarnings = !showWarnings;
      localStorage.setItem(`${DIAG_FILTER_KEY}.warning`, showWarnings ? "1" : "0");
    }
    applyDiagFilters();
    renderDiagnostics(lastDiags);
  });
}
applyDiagFilters();

// Click a problem to jump the cursor to it.
diagnosticsEl.addEventListener("click", (ev) => {
  const el = (ev.target as HTMLElement).closest(".diag") as HTMLElement | null;
  const line = Number(el?.dataset.line);
  if (line) {
    editor.revealLineInCenter(line);
    editor.setPosition({ lineNumber: line, column: 1 });
    editor.focus();
  }
});

function escapeHtml(s: string): string {
  return s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]!);
}

// The compiler is fast, but there's no need to run it on every keystroke.
let timer: number | undefined;
editor.onDidChangeModelContent(() => {
  window.clearTimeout(timer);
  timer = window.setTimeout(refresh, 150);
});

function loadExample(index: number): void {
  const ex = EXAMPLES[index];
  if (!ex) return;
  openTab(null, ex.source);
  editor.focus();
}

// --- Run -------------------------------------------------------------------

const runAsideBtn = document.getElementById("run-aside") as HTMLButtonElement;
const runFileBtn = document.getElementById("run-file") as HTMLButtonElement;
const runRailBtn = document.getElementById("run-rail") as HTMLButtonElement;
const consoleEl = document.getElementById("console")!;
const runButtons = [runAsideBtn, runFileBtn, runRailBtn];

/** Path to hand `vbr runproject`: a saved .vbr, else the open folder if it has main.vbr. */
function projectRunPath(): string | null {
  const tab = activeTab();
  if (tab?.path && /\.vbr$/i.test(tab.path)) return tab.path;
  if (projectIsVbr && projectRoot) return projectRoot;
  return null;
}

function showTool(id: ToolId): void {
  toolOpen = true;
  activeTool = id;
  localStorage.setItem(TOOL_KEY, id);
  applyTool();
}

function revealOutput(): void {
  if (toolOpen && activeTool === "output") return;
  showTool("output");
}

function appendRunLog(chunk: string): void {
  consoleEl.textContent += chunk;
  consoleEl.scrollTop = consoleEl.scrollHeight;
}

/** Stream `vbr runproject` / `vbr test` lines into Output as cargo works. */
async function invokeWithRunLog(
  cmd: string,
  args: Record<string, unknown>,
): Promise<RunOutput> {
  const unlisten = await listen<string>("vbr-run-log", (e) => {
    appendRunLog(e.payload);
  });
  try {
    return await invoke<RunOutput>(cmd, args);
  } finally {
    unlisten();
  }
}

async function runNow(fileOnly: boolean): Promise<void> {
  const runPath = fileOnly ? null : projectRunPath();
  // Nothing to run for a lone non-Vinyl file (a config file, say).
  if (!runPath && !isVbrTab(activeTab())) {
    revealOutput();
    consoleEl.className = "err";
    consoleEl.textContent = "This isn't a Vinyl file — nothing to run.";
    return;
  }
  // runproject reads the files on disk, so offer to save unsaved tabs first.
  if (runPath) {
    const dirty = tabs.filter((t) => t.dirty && t.path);
    if (
      dirty.length > 0 &&
      window.confirm(
        `${dirty.length} open file(s) have unsaved changes. Run uses the ` +
          `saved files — save them first?`,
      )
    ) {
      for (const t of dirty) await saveTab(t, false);
    }
  }
  revealOutput();
  for (const btn of runButtons) btn.disabled = true;
  consoleEl.className = "";
  const label = TARGET_LABELS[currentTarget] ?? "Rust";
  consoleEl.textContent = runPath
    ? "Building and running the project…\n"
    : `Compiling and running (${label})…`;
  try {
    const out = runPath
      ? await invokeWithRunLog("run_project_at", { root: runPath })
      : await invoke<RunOutput>("run_source", {
          source: editor.getValue(),
          target: currentTarget,
        });
    renderRunOutput(out, Boolean(runPath));
  } catch (e) {
    consoleEl.className = "err";
    consoleEl.textContent = String(e);
  } finally {
    for (const btn of runButtons) btn.disabled = false;
  }
}

function runProgram(): void {
  void runNow(false);
}

function runFile(): void {
  void runNow(true);
}

function renderRunOutput(out: RunOutput, keepLog = false): void {
  if (out.stage === "diagnostics") {
    consoleEl.className = "err";
    consoleEl.textContent = "✘ Fix the errors above before running.";
    return;
  }
  if (out.stage === "project") {
    consoleEl.className = "";
    consoleEl.textContent = "ℹ " + out.stderr;
    return;
  }
  if (out.stage === "compile") {
    consoleEl.className = "err";
    consoleEl.textContent = "The generated code did not compile:\n\n" + out.stderr;
    return;
  }
  consoleEl.className = out.success ? "ok" : "err";
  // Project/test runs already streamed cargo + program lines; don't reshuffle them.
  if (keepLog && (consoleEl.textContent ?? "").trim().length > 0) {
    consoleEl.scrollTop = consoleEl.scrollHeight;
    return;
  }
  const body = [out.stderr, out.stdout].filter(Boolean).join("\n").trimEnd();
  consoleEl.textContent = body || "(the program produced no output)";
}

runAsideBtn.addEventListener("click", runProgram);
runRailBtn.addEventListener("click", runProgram);
runFileBtn.addEventListener("click", runFile);
// Ctrl/Cmd+Enter runs from anywhere in the editor.
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, runProgram);

// --- Copy Rust -------------------------------------------------------------

const copyBtn = document.getElementById("copy-rust") as HTMLButtonElement;
copyBtn.addEventListener("click", async () => {
  await navigator.clipboard.writeText(rustView.getValue());
  const label = copyBtn.querySelector("span");
  if (!label) return;
  const prev = label.textContent;
  label.textContent = "Copied";
  window.setTimeout(() => {
    label.textContent = prev;
  }, 1200);
});

// --- File: New / Open / Save -----------------------------------------------

let currentPath: string | null = null;
const statusFile = document.getElementById("status-file")!;
const newBtn = document.getElementById("new-file") as HTMLButtonElement;
const openBtn = document.getElementById("open-file") as HTMLButtonElement;
const saveBtn = document.getElementById("save-file") as HTMLButtonElement;
const saveAsBtn = document.getElementById("saveas-file") as HTMLButtonElement;

function updateFilename(): void {
  const tab = activeTab();
  const name = tab?.path ? basename(tab.path) : "untitled";
  const mark = tab?.dirty ? "● " : "";
  statusFile.textContent = mark + name;
  document.title = `${mark}${name} — Vinyl IDE`;
}

async function saveTab(tab: Tab, forceDialog: boolean): Promise<boolean> {
  const path = await invoke<string | null>("save_file", {
    path: forceDialog ? null : tab.path,
    content: tab.model.getValue(),
    suggested: tab.path ? basename(tab.path) : "untitled.vbr",
  });
  if (!path) return false;
  if (tab.path !== path) {
    tab.path = path;
    monaco.editor.setModelLanguage(tab.model, langForPath(path));
  }
  tab.dirty = false;
  if (tab.id === activeId) {
    currentPath = path;
    void refresh(); // the extension may have changed Vinyl-ness
  }
  renderTabs();
  updateFilename();
  return true;
}

async function openFile(): Promise<boolean> {
  const res = await invoke<OpenedFile | null>("open_file");
  if (!res) return false;
  openTab(res.path, res.content, false);
  return true;
}

async function saveFile(forceDialog: boolean): Promise<void> {
  const tab = activeTab();
  if (!tab) return;
  if (await saveTab(tab, forceDialog)) {
    const original = saveBtn.textContent;
    saveBtn.textContent = "Saved ✓";
    window.setTimeout(() => (saveBtn.textContent = original), 1000);
  }
}

function newFile(): void {
  openTab(null, "");
}

newBtn.addEventListener("click", newFile);
openBtn.addEventListener("click", openFile);
saveBtn.addEventListener("click", () => saveFile(false));
saveAsBtn.addEventListener("click", () => saveFile(true));

editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => saveFile(false));
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyMod.Shift | monaco.KeyCode.KeyS, () => saveFile(true));
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyO, openFile);
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyN, newFile);
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyW, () => {
  if (activeId >= 0) closeTab(activeId);
});

// --- Project / file tree ---------------------------------------------------

let projectRoot: string | null = null;
let projectIsVbr = false; // the open folder has a main.vbr
let isProject = false; // Run should build the project, not the scratch buffer
const sidebar = document.getElementById("sidebar")!;
const sidebarTitle = document.getElementById("sidebar-title")!;
const filetree = document.getElementById("filetree")!;
const openFolderBtn = document.getElementById("open-folder") as HTMLButtonElement;

async function openTreeFile(path: string, el: HTMLElement): Promise<void> {
  const content = await invoke<string>("read_file_at", { path });
  // A file inside a Vinyl project counts as a project file (Run builds the project).
  openTab(path, content, projectIsVbr && path.toLowerCase().endsWith(".vbr"));
  filetree.querySelectorAll(".tree-item.active").forEach((n) => n.classList.remove("active"));
  el.classList.add("active");
}

function renderTree(entries: FileEntry[]): void {
  filetree.innerHTML = "";
  const build = (list: FileEntry[], depth: number, parent: HTMLElement) => {
    for (const entry of list) {
      const row = document.createElement("div");
      row.className = "tree-item" + (entry.is_dir ? " dir" : "");
      row.style.paddingLeft = `${8 + depth * 12}px`;
      if (entry.is_dir) {
        // A collapsible folder: clicking toggles its children.
        row.textContent = "▾ " + entry.name;
        const kids = document.createElement("div");
        build(entry.children, depth + 1, kids);
        row.addEventListener("click", () => {
          const hidden = kids.style.display === "none";
          kids.style.display = hidden ? "" : "none";
          row.textContent = (hidden ? "▾ " : "▸ ") + entry.name;
        });
        parent.append(row, kids);
      } else {
        row.textContent = entry.name;
        row.dataset.path = entry.path;
        row.addEventListener("click", () => openTreeFile(entry.path, row));
        parent.appendChild(row);
      }
    }
  };
  build(entries, 0, filetree);
}

function closeMenu(): void {
  document.querySelectorAll(".context-menu").forEach((m) => m.remove());
}

interface MenuItem {
  label: string;
  danger?: boolean;
  action: () => void;
}

function showMenu(e: MouseEvent, items: MenuItem[]): void {
  closeMenu();
  const menu = document.createElement("div");
  menu.className = "context-menu";
  menu.style.left = `${e.clientX}px`;
  menu.style.top = `${e.clientY}px`;
  for (const it of items) {
    const el = document.createElement("div");
    el.className = "context-item" + (it.danger ? " danger" : "");
    el.textContent = it.label;
    el.addEventListener("click", () => {
      closeMenu();
      it.action();
    });
    menu.appendChild(el);
  }
  document.body.appendChild(menu);
}

async function deleteFile(path: string): Promise<void> {
  const name = path.split(/[/\\]/).pop();
  if (!window.confirm(`Delete ${name}? This cannot be undone.`)) return;
  try {
    await invoke("delete_file", { path });
    const open = tabs.find((t) => t.path === path);
    if (open) {
      open.dirty = false; // it's gone — don't prompt to save
      closeTab(open.id);
    }
    await refreshTree();
  } catch (err) {
    window.alert(String(err));
  }
}

// One context-menu policy: kill the default webview menu (its Reload wiped the
// whole session) and offer a sensible action only where one makes sense.
document.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  const t = e.target as HTMLElement;
  const fileRow = t.closest?.(".tree-item[data-path]") as HTMLElement | null;
  if (fileRow?.dataset.path) {
    const path = fileRow.dataset.path;
    showMenu(e, [{ label: "Delete file", danger: true, action: () => deleteFile(path) }]);
    return;
  }
  const diag = t.closest?.(".diag") as HTMLElement | null;
  if (diag) {
    showMenu(e, [
      { label: "Copy message", action: () => navigator.clipboard.writeText(diag.textContent ?? "") },
    ]);
    return;
  }
  if (t.closest?.("#console")) {
    showMenu(e, [
      { label: "Copy output as text", action: () => navigator.clipboard.writeText(consoleEl.textContent ?? "") },
    ]);
    return;
  }
  // Anywhere else (empty folder area, pane labels…): no menu.
});
window.addEventListener("click", closeMenu);

async function openFolder(): Promise<boolean> {
  const proj = await invoke<Project | null>("open_folder");
  if (!proj) return false;
  projectRoot = proj.root;
  projectIsVbr = proj.is_project;
  isProject = proj.is_project;
  sidebarTitle.textContent = (proj.is_project ? "▣ " : "") + proj.name;
  setSidebarVisible(true);
  renderTree(proj.files);
  // A project opens on its entry point.
  if (proj.entry) {
    const el = filetree.querySelector(`[data-path="${CSS.escape(proj.entry)}"]`) as HTMLElement | null;
    if (el) openTreeFile(proj.entry, el);
  }
  updateProjectButtons();
  return true;
}

openFolderBtn.addEventListener("click", () => void openFolder());

// --- Project actions: Test & Graduate --------------------------------------

const testBtn = document.getElementById("test") as HTMLButtonElement;
const graduateBtn = document.getElementById("graduate") as HTMLButtonElement;

function updateProjectButtons(): void {
  testBtn.disabled = !projectRoot;
  graduateBtn.disabled = !(isProject && !!currentPath && currentPath.endsWith(".vbr"));
}

async function refreshTree(): Promise<void> {
  if (!projectRoot) return;
  const proj = await invoke<Project>("read_project_at", { root: projectRoot });
  renderTree(proj.files);
}

async function testProgram(): Promise<void> {
  if (!projectRoot) return;
  revealOutput();
  testBtn.disabled = true;
  consoleEl.className = "";
  consoleEl.textContent = "Running tests…\n";
  try {
    renderRunOutput(await invokeWithRunLog("test_at", { root: projectRoot }), true);
  } catch (e) {
    consoleEl.className = "err";
    consoleEl.textContent = String(e);
  } finally {
    updateProjectButtons();
  }
}

async function graduateProgram(): Promise<void> {
  if (!currentPath) return;
  const name = currentPath.split(/[/\\]/).pop();
  if (
    !window.confirm(
      `Graduate ${name}?\n\nThis promotes its generated Rust to source and ` +
        `retires the .vbr (kept beside it as .vbr.graduated).`,
    )
  ) {
    return;
  }
  graduateBtn.disabled = true;
  graduateBtn.textContent = "Graduating…";
  consoleEl.className = "";
  consoleEl.textContent = `Graduating ${name}…`;
  try {
    const out = await invoke<RunOutput>("graduate_at", { path: currentPath });
    renderRunOutput(out);
    if (out.success) await refreshTree(); // the files on disk changed
  } catch (e) {
    consoleEl.className = "err";
    consoleEl.textContent = String(e);
  } finally {
    graduateBtn.textContent = "Graduate";
    updateProjectButtons();
  }
}

testBtn.addEventListener("click", testProgram);
graduateBtn.addEventListener("click", graduateProgram);

// --- Tool window (Rust / Output / Problems), toggled from the left rail ----

type ToolId = "rust" | "output" | "problems";
const TOOL_KEY = "vbr-ide.tool";
const toolBtns = [...document.querySelectorAll<HTMLButtonElement>(".tool-btn")];
let toolOpen = true;
let activeTool: ToolId = "rust";

function applyTool(): void {
  document.body.classList.toggle("tool-hidden", !toolOpen);
  for (const btn of toolBtns) {
    btn.classList.toggle("active", toolOpen && btn.dataset.tool === activeTool);
  }
  for (const view of document.querySelectorAll<HTMLElement>(".tool-view")) {
    view.classList.toggle("active", view.dataset.tool === activeTool);
  }
  for (const icons of document.querySelectorAll<HTMLElement>(".aside-icons")) {
    icons.classList.toggle("active", icons.dataset.tool === activeTool);
  }
  requestAnimationFrame(() => {
    editor.layout();
    rustView.layout();
  });
}

function selectTool(next: ToolId): void {
  if (toolOpen && activeTool === next) {
    toolOpen = false;
  } else {
    toolOpen = true;
    activeTool = next;
  }
  localStorage.setItem(TOOL_KEY, toolOpen ? activeTool : "hidden");
  applyTool();
}

for (const btn of toolBtns) {
  btn.addEventListener("click", () => selectTool(btn.dataset.tool as ToolId));
}

applyTool();

const gutterBottom = document.getElementById("gutter-bottom")!;
const ideMain = document.getElementById("ide-main")!;
let draggingBottomH = false;

gutterBottom.addEventListener("mousedown", () => {
  draggingBottomH = true;
  document.body.classList.add("resizing");
});
window.addEventListener("mousemove", (e) => {
  if (draggingBottomH) {
    const rect = ideMain.getBoundingClientRect();
    const h = Math.min(rect.height * 0.8, Math.max(80, rect.bottom - e.clientY));
    ideMain.style.setProperty("--bottom-height", `${h}px`);
  }
});
window.addEventListener("mouseup", () => {
  if (draggingBottomH) {
    editor.layout();
    rustView.layout();
  }
  draggingBottomH = false;
  document.body.classList.remove("resizing");
});

// One left column (files + tool menus).
const gutterSidebar = document.getElementById("gutter-sidebar")!;
const toggleSidebarBtn = document.getElementById("toggle-sidebar") as HTMLButtonElement;
const SIDEBAR_KEY = "vbr-ide.sidebar";
let draggingSidebar = false;

function setSidebarVisible(show: boolean): void {
  document.body.classList.toggle("sidebar-hidden", !show);
  toggleSidebarBtn.classList.toggle("active", show);
  localStorage.setItem(SIDEBAR_KEY, show ? "1" : "0");
  requestAnimationFrame(() => {
    editor.layout();
    rustView.layout();
  });
}

toggleSidebarBtn.addEventListener("click", () => {
  setSidebarVisible(document.body.classList.contains("sidebar-hidden"));
});
if (localStorage.getItem(SIDEBAR_KEY) === "1") {
  setSidebarVisible(true);
}

gutterSidebar.addEventListener("mousedown", () => {
  draggingSidebar = true;
  document.body.classList.add("resizing");
});
window.addEventListener("mousemove", (e) => {
  if (draggingSidebar) {
    const rect = ideMain.getBoundingClientRect();
    const w = Math.min(rect.width * 0.6, Math.max(40, e.clientX - rect.left));
    ideMain.style.setProperty("--sidebar-width", `${w}px`);
  }
});
window.addEventListener("mouseup", () => {
  if (draggingSidebar) {
    editor.layout();
    rustView.layout();
  }
  draggingSidebar = false;
  document.body.classList.remove("resizing");
});

// --- Help overlay ----------------------------------------------------------

const helpBtn = document.getElementById("help") as HTMLButtonElement;
const helpOverlay = document.getElementById("help-overlay")!;

function toggleHelp(show: boolean): void {
  helpOverlay.classList.toggle("hidden", !show);
}

helpBtn.addEventListener("click", () => toggleHelp(true));
helpOverlay.addEventListener("click", () => toggleHelp(false));
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    toggleHelp(false);
    closeAppMenu();
  } else if (e.key === "?" && !editor.hasTextFocus() && !rustView.hasTextFocus()) {
    toggleHelp(true);
  }
});

// --- Theme -----------------------------------------------------------------

const themeBtn = document.getElementById("theme") as HTMLButtonElement;
const THEME_KEY = "vbr-ide.theme";

function applyTheme(light: boolean): void {
  document.body.classList.toggle("light", light);
  localStorage.setItem(THEME_KEY, light ? "light" : "dark");
  defineMonacoThemes();
}

themeBtn.addEventListener("click", () => {
  applyTheme(!document.body.classList.contains("light"));
});
applyTheme(localStorage.getItem(THEME_KEY) === "light");

// --- Form designer (separate window) ---------------------------------------

const enterDesignerBtn = document.getElementById("enter-designer") as HTMLButtonElement;
const enterScreenBtn = document.getElementById("enter-screen") as HTMLButtonElement;

async function enterDesigner(t: "gui" | "tui"): Promise<void> {
  if (!projectRoot) await openFolder();
  if (!projectRoot) return;
  try {
    await invoke("open_designer", { target: t, root: projectRoot });
  } catch (e) {
    window.alert(String(e));
  }
}

enterDesignerBtn.addEventListener("click", () => void enterDesigner("gui"));
enterScreenBtn.addEventListener("click", () => void enterDesigner("tui"));

void listen<{ path: string; name: string }>("designer-form-created", async (e) => {
  await refreshTree();
  const content = await invoke<string>("read_file_at", { path: e.payload.path });
  openTab(e.payload.path, content, projectIsVbr);
});

void listen("designer-closing", () => {
  void getCurrentWebviewWindow().setFocus();
});

function enterIde(): void {
  document.body.classList.remove("splash");
  toolOpen = true;
  activeTool = "rust";
  applyTool();
  if (tabs.length === 0) openTab(null, "");
  requestAnimationFrame(() => {
    editor.layout();
    rustView.layout();
    editor.focus();
  });
}

document.getElementById("splash-folder")!.addEventListener("click", async () => {
  if (await openFolder()) enterIde();
});
document.getElementById("splash-file")!.addEventListener("click", async () => {
  if (await openFile()) enterIde();
});

{
  const splashTiles = document.getElementById("splash-tiles")!;
  const splashExamples = document.getElementById("splash-examples")!;
  const splashList = document.getElementById("splash-example-list")!;
  document.getElementById("splash-example")!.addEventListener("click", () => {
    if (!splashList.childElementCount) {
      let last = "";
      EXAMPLES.forEach((ex) => {
        if (ex.group !== last) {
          const h = document.createElement("div");
          h.className = "splash-ex-group";
          h.textContent = ex.group;
          splashList.appendChild(h);
          last = ex.group;
        }
        const b = document.createElement("button");
        b.type = "button";
        b.className = "splash-ex-item";
        b.textContent = ex.label;
        b.addEventListener("click", () => {
          openTab(null, ex.source);
          enterIde();
        });
        splashList.appendChild(b);
      });
    }
    splashTiles.classList.add("hidden");
    splashExamples.classList.remove("hidden");
  });
  document.getElementById("splash-examples-back")!.addEventListener("click", () => {
    splashExamples.classList.add("hidden");
    splashTiles.classList.remove("hidden");
  });
}

// Tabs are created when the splash is dismissed (file / folder / example / blank).

// --- Application menu (File / Edit / View / Run / Gui) ---------------------

const appMenuBtn = document.getElementById("app-menu-btn") as HTMLButtonElement;
const appMenu = document.getElementById("app-menu")!;
const appSubmenu = document.getElementById("app-submenu")!;
let openSub: string | null = null;

function closeAppMenu(): void {
  appMenu.classList.add("hidden");
  appSubmenu.classList.add("hidden");
  appSubmenu.replaceChildren();
  appMenuBtn.setAttribute("aria-expanded", "false");
  openSub = null;
  for (const el of appMenu.querySelectorAll(".menu-item.open")) el.classList.remove("open");
}

function placeMenu(el: HTMLElement, x: number, y: number): void {
  el.style.left = `${x}px`;
  el.style.top = `${y}px`;
}

function menuItem(label: string, action: () => void, extra?: { kbd?: string; disabled?: boolean }): HTMLButtonElement {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "menu-item";
  b.append(label);
  if (extra?.kbd) {
    const k = document.createElement("span");
    k.textContent = extra.kbd;
    b.append(k);
  }
  if (extra?.disabled) b.disabled = true;
  b.addEventListener("click", () => {
    if (b.disabled) return;
    closeAppMenu();
    action();
  });
  return b;
}

function menuHead(label: string): HTMLDivElement {
  const d = document.createElement("div");
  d.className = "menu-head";
  d.textContent = label;
  return d;
}

function fillSubmenu(which: string): void {
  const box = appSubmenu;
  box.replaceChildren();
  if (which === "file") {
    box.append(
      menuItem("New", () => newBtn.click(), { kbd: "Ctrl+N" }),
      menuItem("Open file…", () => openBtn.click(), { kbd: "Ctrl+O" }),
      menuItem("Open folder…", () => openFolderBtn.click()),
      menuItem("Save", () => saveBtn.click(), { kbd: "Ctrl+S" }),
      menuItem("Save As…", () => saveAsBtn.click(), { kbd: "Ctrl+Shift+S" }),
    );
  } else if (which === "examples") {
    let last = "";
    EXAMPLES.forEach((ex, i) => {
      if (ex.group !== last) {
        box.append(menuHead(ex.group));
        last = ex.group;
      }
      box.append(menuItem(ex.label, () => loadExample(i)));
    });
  } else if (which === "edit") {
    box.append(
      menuItem("Copy generated output", () => copyBtn.click()),
      menuItem("Find", () => void editor.getAction("actions.find")?.run(), { kbd: "Ctrl+F" }),
    );
  } else if (which === "view") {
    box.append(
      menuItem("Left pane", () => toggleSidebarBtn.click()),
      menuItem("Rust", () => showTool("rust")),
      menuItem("Output", () => showTool("output")),
      menuItem("Problems", () => showTool("problems")),
    );
  } else if (which === "run") {
    box.append(
      menuItem("Run", () => runProgram(), { kbd: "Ctrl+Enter" }),
      menuItem("Run file", () => runFile()),
      menuItem("Test", () => void testProgram(), { disabled: !projectRoot }),
      menuItem("Graduate", () => void graduateProgram(), {
        disabled: graduateBtn.disabled,
      }),
    );
  } else if (which === "gui") {
    box.append(
      menuItem("New Form", () => enterDesignerBtn.click()),
      menuItem("New Screen", () => enterScreenBtn.click()),
    );
  }
}

function openSubmenu(which: string, from: HTMLElement): void {
  openSub = which;
  for (const el of appMenu.querySelectorAll(".menu-item")) {
    el.classList.toggle("open", (el as HTMLElement).dataset.sub === which);
  }
  fillSubmenu(which);
  appSubmenu.classList.remove("hidden");
  const r = appMenu.getBoundingClientRect();
  const row = from.getBoundingClientRect();
  placeMenu(appSubmenu, r.right + 4, row.top);
}

function openAppMenu(): void {
  const r = appMenuBtn.getBoundingClientRect();
  placeMenu(appMenu, r.right + 6, r.top);
  appMenu.classList.remove("hidden");
  appMenuBtn.setAttribute("aria-expanded", "true");
  const first = appMenu.querySelector<HTMLElement>("[data-sub='file']");
  if (first) openSubmenu("file", first);
}

appMenuBtn.addEventListener("click", (e) => {
  e.stopPropagation();
  if (!appMenu.classList.contains("hidden")) closeAppMenu();
  else openAppMenu();
});

for (const btn of appMenu.querySelectorAll<HTMLButtonElement>(".has-sub")) {
  btn.addEventListener("mouseenter", () => {
    if (appMenu.classList.contains("hidden")) return;
    openSubmenu(btn.dataset.sub ?? "file", btn);
  });
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    openSubmenu(btn.dataset.sub ?? "file", btn);
  });
}

document.addEventListener("click", (e) => {
  const t = e.target as Node;
  if (appMenu.contains(t) || appSubmenu.contains(t) || appMenuBtn.contains(t)) return;
  closeAppMenu();
});
