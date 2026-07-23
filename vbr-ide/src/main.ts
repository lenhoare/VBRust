import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import { invoke } from "@tauri-apps/api/core";
import { registerVbrLanguage, VBR_LANGUAGE_ID } from "./vbrLanguage";
import { EXAMPLES } from "./examples";

// Monaco needs a worker for the editor itself; VBR and Rust are both
// Monarch-tokenised on the main thread here, so the base editor worker is all
// we wire up. (Real VBR tokenisation lands in slice 6.)
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
  rust: string;
  diagnostics: Diagnostic[];
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

const SAMPLE = `' Welcome to VBR — type on the left, read the Rust on the right.
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

// Restore the last session's work if there is any, otherwise show the welcome.
const STORAGE_KEY = "vbr-ide.source";
const initialSource = localStorage.getItem(STORAGE_KEY) ?? SAMPLE;

const editor = monaco.editor.create(document.getElementById("editor")!, {
  value: initialSource,
  language: VBR_LANGUAGE_ID,
  theme: "vs-dark",
  minimap: { enabled: false },
  fontSize: 14,
  automaticLayout: true,
  scrollBeyondLastLine: false,
  mouseWheelZoom: true,
});

const rustView = monaco.editor.create(document.getElementById("rust")!, {
  value: "",
  language: "rust",
  theme: "vs-dark",
  readOnly: true,
  minimap: { enabled: false },
  fontSize: 14,
  automaticLayout: true,
  scrollBeyondLastLine: false,
  mouseWheelZoom: true,
});

const diagnosticsEl = document.getElementById("diagnostics")!;

async function refresh(): Promise<void> {
  const source = editor.getValue();
  try {
    const t0 = performance.now();
    const result = await invoke<TranspileResult>("transpile_source", { source });
    const ms = Math.max(1, Math.round(performance.now() - t0));
    // Preserve the reader's scroll position when only the text changed.
    rustView.setValue(result.rust);
    renderDiagnostics(result.diagnostics);
    setMarkers(result.diagnostics);
    updateStatus(result.diagnostics, ms);
  } catch (e) {
    diagnosticsEl.textContent = String(e);
  }
}

const statusPos = document.getElementById("status-pos")!;
const statusProblems = document.getElementById("status-problems")!;
const statusTiming = document.getElementById("status-timing")!;

editor.onDidChangeCursorPosition((e) => {
  statusPos.textContent = `Ln ${e.position.lineNumber}, Col ${e.position.column}`;
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

// Paint the diagnostics as inline squiggles on the VBR pane. A diagnostic with
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

function renderDiagnostics(diags: Diagnostic[]): void {
  if (diags.length === 0) {
    diagnosticsEl.innerHTML = `<span class="ok">✓ no diagnostics</span>`;
    return;
  }
  diagnosticsEl.innerHTML = diags
    .map((d) => {
      const icon = d.level === "error" ? "✘" : d.level === "warning" ? "⚠" : "ℹ";
      const line = d.range?.startLineNumber ?? d.line ?? 0;
      const where = line ? `line ${line}: ` : "";
      const attr = line ? ` data-line="${line}"` : "";
      return `<div class="diag ${d.level}"${attr}>${icon} ${where}${escapeHtml(d.message)}</div>`;
    })
    .join("");
}

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
  localStorage.setItem(STORAGE_KEY, editor.getValue());
  window.clearTimeout(timer);
  timer = window.setTimeout(refresh, 150);
});

// --- Example picker --------------------------------------------------------

const exampleSelect = document.getElementById("examples") as HTMLSelectElement;
for (const ex of EXAMPLES) {
  const opt = document.createElement("option");
  opt.value = ex.label;
  opt.textContent = ex.label;
  exampleSelect.appendChild(opt);
}
exampleSelect.addEventListener("change", () => {
  const ex = EXAMPLES.find((e) => e.label === exampleSelect.value);
  if (ex) {
    editor.setValue(ex.source);
    editor.focus();
  }
  exampleSelect.value = ""; // reset to the "Load example…" placeholder
});

// --- Run -------------------------------------------------------------------

const runBtn = document.getElementById("run") as HTMLButtonElement;
const consoleEl = document.getElementById("console")!;

async function runProgram(): Promise<void> {
  runBtn.disabled = true;
  runBtn.textContent = "▶ Running…";
  consoleEl.className = "";
  consoleEl.textContent = "Compiling and running…";
  try {
    const out = await invoke<RunOutput>("run_source", { source: editor.getValue() });
    renderRunOutput(out);
  } catch (e) {
    consoleEl.className = "err";
    consoleEl.textContent = String(e);
  } finally {
    runBtn.disabled = false;
    runBtn.textContent = "▶ Run";
  }
}

function renderRunOutput(out: RunOutput): void {
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
    consoleEl.textContent = "The generated Rust did not compile:\n\n" + out.stderr;
    return;
  }
  const body = [out.stdout, out.stderr].filter(Boolean).join("\n").trimEnd();
  consoleEl.className = out.success ? "ok" : "err";
  consoleEl.textContent = body || "(the program produced no output)";
}

runBtn.addEventListener("click", runProgram);
// Ctrl/Cmd+Enter runs from anywhere in the editor.
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, runProgram);

// --- Copy Rust -------------------------------------------------------------

const copyBtn = document.getElementById("copy-rust") as HTMLButtonElement;
copyBtn.addEventListener("click", async () => {
  await navigator.clipboard.writeText(rustView.getValue());
  const original = copyBtn.textContent;
  copyBtn.textContent = "Copied ✓";
  window.setTimeout(() => (copyBtn.textContent = original), 1200);
});

// --- File: New / Open / Save -----------------------------------------------

let currentPath: string | null = null;
const statusFile = document.getElementById("status-file")!;
const newBtn = document.getElementById("new-file") as HTMLButtonElement;
const openBtn = document.getElementById("open-file") as HTMLButtonElement;
const saveBtn = document.getElementById("save-file") as HTMLButtonElement;

function updateFilename(): void {
  const name = currentPath ? currentPath.split(/[/\\]/).pop()! : "untitled";
  statusFile.textContent = name;
  document.title = `${name} — VBR IDE`;
}

async function openFile(): Promise<void> {
  const res = await invoke<OpenedFile | null>("open_file");
  if (res) {
    editor.setValue(res.content);
    currentPath = res.path;
    updateFilename();
  }
}

async function saveFile(forceDialog: boolean): Promise<void> {
  const path = await invoke<string | null>("save_file", {
    path: forceDialog ? null : currentPath,
    content: editor.getValue(),
  });
  if (path) {
    currentPath = path;
    updateFilename();
    const original = saveBtn.textContent;
    saveBtn.textContent = "Saved ✓";
    window.setTimeout(() => (saveBtn.textContent = original), 1000);
  }
}

function newFile(): void {
  editor.setValue("");
  currentPath = null;
  updateFilename();
  editor.focus();
}

newBtn.addEventListener("click", newFile);
openBtn.addEventListener("click", openFile);
saveBtn.addEventListener("click", () => saveFile(false));

editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => saveFile(false));
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyO, openFile);
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyN, newFile);
updateFilename();

// --- Resizable split -------------------------------------------------------

const gutter = document.getElementById("gutter")!;
const leftPane = document.getElementById("left-pane")!;
const panesEl = document.getElementById("panes")!;
const SPLIT_KEY = "vbr-ide.split";
let dragging = false;

const savedSplit = localStorage.getItem(SPLIT_KEY);
if (savedSplit) leftPane.style.flexBasis = savedSplit;

gutter.addEventListener("mousedown", () => {
  dragging = true;
  panesEl.classList.add("dragging");
});
window.addEventListener("mousemove", (e) => {
  if (!dragging) return;
  const rect = panesEl.getBoundingClientRect();
  const pct = Math.min(85, Math.max(15, ((e.clientX - rect.left) / rect.width) * 100));
  leftPane.style.flexBasis = `${pct}%`;
});
window.addEventListener("mouseup", () => {
  if (dragging) localStorage.setItem(SPLIT_KEY, leftPane.style.flexBasis);
  dragging = false;
  panesEl.classList.remove("dragging");
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
  } else if (e.key === "?" && !editor.hasTextFocus() && !rustView.hasTextFocus()) {
    toggleHelp(true);
  }
});

// --- Theme -----------------------------------------------------------------

const themeBtn = document.getElementById("theme") as HTMLButtonElement;
const THEME_KEY = "vbr-ide.theme";

function applyTheme(light: boolean): void {
  document.body.classList.toggle("light", light);
  monaco.editor.setTheme(light ? "vs" : "vs-dark");
  localStorage.setItem(THEME_KEY, light ? "light" : "dark");
}

themeBtn.addEventListener("click", () => {
  applyTheme(!document.body.classList.contains("light"));
});
applyTheme(localStorage.getItem(THEME_KEY) === "light");

refresh();
