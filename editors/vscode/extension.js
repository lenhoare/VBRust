// VS Code integration for VBR. Two features live here:
//
//  1. The language client — launches the `vbr-lsp` binary over stdio; the server
//     runs the real compiler and reports diagnostics/hover/completion/defs.
//  2. A live "Rust output" side view — a read-only virtual document that shows
//     the Rust the current .vbr transpiles to, refreshed on save. This is the
//     transpiler's whole point made visible: VB on the left, idiomatic Rust on
//     the right.

const path = require("path");
const fs = require("fs");
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

// Run `vbr emit <file>` and resolve to the Rust it prints. On failure, resolve
// to the transpiler's diagnostics as a comment block (so the pane still helps
// while you fix errors). `emit` reads the file from disk — callers save first.
function emitRust(sourcePath) {
  return new Promise((resolve) => {
    const root = rootFor(sourcePath);
    const opts = { maxBuffer: 16 * 1024 * 1024, cwd: root };
    const bin = vbrBinary(root);
    let cmd, args;
    if (bin) {
      cmd = bin;
      args = ["emit", sourcePath];
    } else {
      cmd = process.platform === "win32" ? "cargo.exe" : "cargo";
      args = [
        "run", "--quiet",
        "--manifest-path", path.join(root, "Cargo.toml"),
        "--", "emit", sourcePath,
      ];
    }
    execFile(cmd, args, opts, (err, stdout, stderr) => {
      if (!err && stdout) {
        resolve(stdout);
        return;
      }
      const header =
        "// ── VBR could not produce Rust for this file ──\n" +
        "// (transpiler diagnostics below — fix them, then save to refresh)\n\n";
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
function sourcePathFromRustUri(uri) {
  return uri.with({ scheme: "file", path: uri.path.replace(/\.rs$/, "") }).fsPath;
}

// Serves the vbr-rust scheme; fires onDidChange to force a re-fetch on save.
class RustViewProvider {
  constructor() {
    this._onDidChange = new vscode.EventEmitter();
    this.onDidChange = this._onDidChange.event;
  }
  refresh(uri) {
    this._onDidChange.fire(uri);
  }
  async provideTextDocumentContent(uri) {
    return emitRust(sourcePathFromRustUri(uri));
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

  async function showRustOutput() {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.languageId !== "vbr") {
      vscode.window.showInformationMessage("Open a .vbr file first.");
      return;
    }
    // emit reads from disk, so flush unsaved edits before showing.
    if (editor.document.isDirty) await editor.document.save();

    const rustUri = rustUriFor(editor.document.uri);
    rustProvider.refresh(rustUri);
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
  }

  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider("vbr-rust", rustProvider),
    vscode.commands.registerCommand("vbr.showRustOutput", showRustOutput),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === "vbr") {
        rustProvider.refresh(rustUriFor(doc.uri));
      }
    })
  );
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
