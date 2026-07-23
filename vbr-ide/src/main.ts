import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import { invoke } from "@tauri-apps/api/core";
import { registerVbrLanguage, VBR_LANGUAGE_ID } from "./vbrLanguage";

// Monaco needs a worker for the editor itself; VBR and Rust are both
// Monarch-tokenised on the main thread here, so the base editor worker is all
// we wire up. (Real VBR tokenisation lands in slice 6.)
self.MonacoEnvironment = {
  getWorker: () => new editorWorker(),
};

interface Diagnostic {
  level: "error" | "warning" | "note";
  message: string;
  line: number | null;
  start: number | null;
  end: number | null;
}

interface TranspileResult {
  rust: string;
  diagnostics: Diagnostic[];
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

const editor = monaco.editor.create(document.getElementById("editor")!, {
  value: SAMPLE,
  language: VBR_LANGUAGE_ID,
  theme: "vs-dark",
  minimap: { enabled: false },
  fontSize: 14,
  automaticLayout: true,
  scrollBeyondLastLine: false,
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
});

const diagnosticsEl = document.getElementById("diagnostics")!;

async function refresh(): Promise<void> {
  const source = editor.getValue();
  try {
    const result = await invoke<TranspileResult>("transpile_source", { source });
    // Preserve the reader's scroll position when only the text changed.
    rustView.setValue(result.rust);
    renderDiagnostics(result.diagnostics);
  } catch (e) {
    diagnosticsEl.textContent = String(e);
  }
}

function renderDiagnostics(diags: Diagnostic[]): void {
  if (diags.length === 0) {
    diagnosticsEl.innerHTML = `<span class="ok">✓ no diagnostics</span>`;
    return;
  }
  diagnosticsEl.innerHTML = diags
    .map((d) => {
      const icon = d.level === "error" ? "✘" : d.level === "warning" ? "⚠" : "ℹ";
      const where = d.line ? `line ${d.line}: ` : "";
      return `<div class="diag ${d.level}">${icon} ${where}${escapeHtml(d.message)}</div>`;
    })
    .join("");
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]!);
}

// The compiler is fast, but there's no need to run it on every keystroke.
let timer: number | undefined;
editor.onDidChangeModelContent(() => {
  window.clearTimeout(timer);
  timer = window.setTimeout(refresh, 150);
});

refresh();
