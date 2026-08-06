// VS Code integration for VBR. Two features live here:
//
//  1. The language client — launches the `vbr-lsp` binary over stdio; the server
//     runs the real compiler and reports diagnostics/hover/completion/defs.
//  2. A live "Rust output" side view — a read-only virtual document that shows
//     the Rust the current .vbr transpiles to, updated live as you type. This is
//     the transpiler's whole point made visible: VB on the left, idiomatic Rust
//     on the right.

const path = require("path");
const fs = require("fs");
const os = require("os");
const { execFile } = require("child_process");
const vscode = require("vscode");
const { workspace } = vscode;
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

// The server binary's name, `.exe` on Windows.
const BIN = process.platform === "win32" ? "vbr-lsp.exe" : "vbr-lsp";

// Look for a built LSP binary inside an open workspace folder — this makes the
// same repo work on Linux and Windows without a machine-specific path setting.
// A VBRust checkout has it at <root>/vbr-lsp/target/release/<BIN>.
function findInWorkspace() {
  const folders = workspace.workspaceFolders || [];
  for (const folder of folders) {
    const root = folder.uri.fsPath;
    for (const profile of ["release", "debug"]) {
      const candidate = path.join(root, "vbr-lsp", "target", profile, BIN);
      if (fs.existsSync(candidate)) return candidate;
    }
  }
  return null;
}

function serverCommand(context) {
  // Priority: explicit setting → env var → the build inside the open workspace
  // → the build alongside the extension (dev/source install).
  const configured = workspace.getConfiguration("vbr").get("serverPath");
  if (configured) return configured;
  if (process.env.VBR_LSP_SERVER) return process.env.VBR_LSP_SERVER;
  const inWorkspace = findInWorkspace();
  if (inWorkspace) return inWorkspace;
  // editors/vscode/ → repo root → vbr-lsp/target/release/<BIN>
  return context.asAbsolutePath(
    path.join("..", "..", "vbr-lsp", "target", "release", BIN)
  );
}

// ── Live Rust output view ──────────────────────────────────────────────────

// The workspace folder that owns a given source file (for the Cargo manifest and
// the prebuilt-binary search). Falls back to the first folder, then the file's
// own directory.
function rootFor(sourcePath) {
  const folder = workspace.getWorkspaceFolder(vscode.Uri.file(sourcePath));
  if (folder) return folder.uri.fsPath;
  const folders = workspace.workspaceFolders || [];
  if (folders.length) return folders[0].uri.fsPath;
  return path.dirname(sourcePath);
}

// A prebuilt `vbr` CLI binary inside the repo, if the user has built one — much
// faster than `cargo run`. Returns null to fall back to cargo.
function vbrBinary(root) {
  const name = process.platform === "win32" ? "vbr.exe" : "vbr";
  for (const profile of ["release", "debug"]) {
    const cand = path.join(root, "target", profile, name);
    if (fs.existsSync(cand)) return cand;
  }
  return null;
}

// The current text of a source file — the live editor buffer if it's open
// (so we reflect unsaved edits), otherwise whatever is on disk.
function sourceText(sourceUri) {
  const open = workspace.textDocuments.find(
    (d) => d.uri.toString() === sourceUri.toString()
  );
  if (open) return open.getText();
  try {
    return fs.readFileSync(sourceUri.fsPath, "utf8");
  } catch (_) {
    return "";
  }
}

// Transpile the live buffer to Rust by writing it to a temp file and running
// `vbr emit`. Resolving to the Rust on success, or the transpiler's diagnostics
// as a comment block on failure (so the pane still helps while you fix errors).
function emitRust(sourceUri) {
  return new Promise((resolve) => {
    const root = rootFor(sourceUri.fsPath);
    const tmp = path.join(
      os.tmpdir(),
      `vbrview-${Date.now()}-${Math.floor(Math.random() * 1e6)}.vbr`
    );
    try {
      fs.writeFileSync(tmp, sourceText(sourceUri));
    } catch (e) {
      resolve("// could not write temp file: " + e.message);
      return;
    }

    const bin = vbrBinary(root);
    let cmd, args;
    if (bin) {
      cmd = bin;
      args = ["emit", tmp];
    } else {
      cmd = process.platform === "win32" ? "cargo.exe" : "cargo";
      args = [
        "run", "--quiet",
        "--manifest-path", path.join(root, "Cargo.toml"),
        "--", "emit", tmp,
      ];
    }

    execFile(cmd, args, { maxBuffer: 16 * 1024 * 1024, cwd: root }, (err, stdout, stderr) => {
      try {
        fs.unlinkSync(tmp);
      } catch (_) {
        /* best effort */
      }
      if (!err && stdout) {
        resolve(stdout);
        return;
      }
      const header =
        "// ── VBR could not produce Rust for this file ──\n" +
        "// (transpiler diagnostics below — fix them and the view refreshes)\n\n";
      const detail = (stderr || (err && err.message) || "unknown error").trimEnd();
      resolve(header + detail.split("\n").map((l) => "// " + l).join("\n"));
    });
  });
}

// Maps a .vbr document URI to its virtual Rust-view URI (same path + ".rs" under
// the vbr-rust scheme), and back. The ".rs" suffix makes VS Code treat the view
// as Rust for highlighting.
function rustUriFor(sourceUri) {
  return sourceUri.with({ scheme: "vbr-rust", path: sourceUri.path + ".rs" });
}
function sourceUriFromRustUri(uri) {
  return uri.with({ scheme: "file", path: uri.path.replace(/\.rs$/, "") });
}

// Serves the vbr-rust scheme; fires onDidChange to force a re-fetch.
class RustViewProvider {
  constructor() {
    this._onDidChange = new vscode.EventEmitter();
    this.onDidChange = this._onDidChange.event;
  }
  refresh(uri) {
    this._onDidChange.fire(uri);
  }
  provideTextDocumentContent(uri) {
    return emitRust(sourceUriFromRustUri(uri));
  }
}

function activate(context) {
  // 1. Language client.
  const command = serverCommand(context);
  const serverOptions = {
    run: { command, transport: TransportKind.stdio },
    debug: { command, transport: TransportKind.stdio },
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "vbr" }],
  };
  client = new LanguageClient(
    "vbr-lsp",
    "VBR Language Server",
    serverOptions,
    clientOptions
  );
  client.start();

  // 2. Live Rust output view.
  const rustProvider = new RustViewProvider();

  // Debounce refreshes so a fast typist doesn't spawn a transpile per keystroke.
  const timers = new Map();
  function scheduleRefresh(sourceUri) {
    const key = sourceUri.toString();
    if (timers.has(key)) clearTimeout(timers.get(key));
    timers.set(
      key,
      setTimeout(() => {
        timers.delete(key);
        rustProvider.refresh(rustUriFor(sourceUri));
      }, 350)
    );
  }

  async function showRustOutput() {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== "vbr") {
      vscode.window.showInformationMessage("Open a .vbr file first.");
      return;
    }
    const rustUri = rustUriFor(editor.document.uri);
    const doc = await vscode.workspace.openTextDocument(rustUri);
    try {
      await vscode.languages.setTextDocumentLanguage(doc, "rust");
    } catch (_) {
      // No Rust language installed — it stays plain text, still readable.
    }
    await vscode.window.showTextDocument(doc, {
      viewColumn: vscode.ViewColumn.Two,
      preserveFocus: true,
      preview: false,
    });
    // Force fresh content now the view is on screen (handles reopen-after-close,
    // where VS Code may have a cached model).
    rustProvider.refresh(rustUri);
  }

  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider("vbr-rust", rustProvider),
    vscode.commands.registerCommand("vbr.showRustOutput", showRustOutput),
    vscode.workspace.onDidChangeTextDocument((e) => {
      if (e.document.languageId === "vbr") scheduleRefresh(e.document.uri);
    }),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === "vbr") rustProvider.refresh(rustUriFor(doc.uri));
    })
  );
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
