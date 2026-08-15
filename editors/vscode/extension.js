// VS Code integration for Bust. Two features live here:
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
        "// ── Bust could not produce Rust for this file ──\n" +
        "// (transpiler diagnostics below — fix them and the view refreshes)\n\n";
      const detail = (stderr || (err && err.message) || "unknown error").trimEnd();
      resolve(header + detail.split("\n").map((l) => "// " + l).join("\n"));
    });
  });
}

// ── Running: `.vbr` files, and `.rs` files with embedded Bust ────────────────

// Shell-quote a single argument for the integrated terminal. VS Code's default
// terminal is PowerShell on Windows, POSIX sh elsewhere; both accept single- or
// double-quoted args, but paths with spaces are the only real hazard.
function quoteArg(a) {
  if (process.platform === "win32") return `"${a}"`;
  return `'${a.replace(/'/g, "'\\''")}'`;
}

// [cmd, args] to invoke the Bust CLI: a prebuilt `vbr` binary if present (fast),
// otherwise `cargo run` from the workspace manifest.
function vbrCmd(root, subArgs) {
  const bin = vbrBinary(root);
  if (bin) return [bin, subArgs];
  const cargo = process.platform === "win32" ? "cargo.exe" : "cargo";
  return [
    cargo,
    ["run", "--quiet", "--manifest-path", path.join(root, "Cargo.toml"), "--", ...subArgs],
  ];
}

// Promise wrapper around execFile — never rejects; resolves {failed,stdout,stderr}.
function execAsync(cmd, args, cwd) {
  return new Promise((resolve) => {
    execFile(cmd, args, { maxBuffer: 16 * 1024 * 1024, cwd }, (err, stdout, stderr) => {
      resolve({ failed: !!err, stdout: stdout || "", stderr: stderr || "" });
    });
  });
}

// Build a shell command line from an executable + args. The wrinkle: PowerShell
// (VS Code's default terminal on Windows) parses a command that *starts* with a
// quoted string as a string literal, not a program to run — it needs the call
// operator `&` in front. cmd and POSIX shells neither need nor tolerate it, so we
// add it only for PowerShell, detected from the configured terminal shell.
function terminalCommand(exe, args) {
  const line = [exe, ...args].map(quoteArg).join(" ");
  const shell = (vscode.env.shell || "").toLowerCase();
  const isPowerShell = shell.includes("powershell") || shell.includes("pwsh");
  return isPowerShell ? "& " + line : line;
}

// One reused "Bust Run" terminal so successive runs don't pile up panels.
let runTerminal;
function runInTerminal(exe, args, cwd) {
  if (!runTerminal || runTerminal.exitStatus !== undefined) {
    runTerminal = vscode.window.createTerminal({ name: "Bust Run", cwd });
  }
  runTerminal.show(true);
  runTerminal.sendText(terminalCommand(exe, args));
}

// An output channel for build errors (rustc / embed) that don't belong in the
// run terminal.
let outputChannel;
function showErrors(title, body) {
  if (!outputChannel) outputChannel = vscode.window.createOutputChannel("Bust");
  outputChannel.clear();
  outputChannel.appendLine(title);
  if (body) outputChannel.append(body);
  outputChannel.show(true);
}

// Run a `.vbr` file: prefers the prebuilt binary, else `cargo run`. The
// no-debugger "click to run" from the ▶ button / Ctrl+Alt+R.
function runVbrFile(sourceUri) {
  const filePath = sourceUri.fsPath;
  const root = rootFor(filePath);
  const [cmd, args] = vbrCmd(root, ["run", filePath]);
  runInTerminal(cmd, args, root);
}

// Run a `.rs` file that embeds Bust: expand the `/* vbr … */` block(s) with
// `vbr embed`, reload the buffer so the regenerated region is visible, then —
// for a standalone file (has `fn main`) — compile with rustc and run it. Inside
// a Cargo crate we stop after expanding and defer to cargo's normal build/run.
// Single file only; no project mode on this path.
async function embedAndRunRust(document) {
  const filePath = document.uri.fsPath;

  // Check on click (cheap) — do nothing if there's no Bust to expand.
  if (!/\/\*\s*vbr\b/.test(document.getText())) {
    vscode.window.showInformationMessage(
      "No `/* vbr … */` block in this file — nothing to embed."
    );
    return;
  }

  // Must be saved: `vbr embed` rewrites the file on disk, so we never race the
  // buffer. Cancel if the save doesn't take.
  if (document.isDirty) {
    const saved = await document.save();
    if (!saved) return;
  }

  const root = rootFor(filePath);

  // 1. Expand embedded Bust.
  const [ec, eargs] = vbrCmd(root, ["embed", filePath]);
  const embed = await execAsync(ec, eargs, root);
  // Reload so the regenerated // vbr:gen region shows (also surfaces any in-file
  // `//` error lines embed writes on a fragment error).
  await vscode.commands.executeCommand("workbench.action.files.revert");
  if (embed.failed) {
    const detail = (embed.stderr || embed.stdout || "").trim();
    showErrors("vbr embed failed:", detail || "(no output from the vbr binary)");
    const firstLine = detail.split("\n").find((l) => l.trim()) || "";
    const hint = /unrecognized|unexpected argument|USAGE|SUBCOMMAND/i.test(detail)
      ? " — your vbr binary looks too old for `embed`; run `cargo build --release`."
      : "";
    vscode.window.showErrorMessage(
      "vbr embed failed: " + (firstLine || "no output") + hint
    );
    return;
  }

  // 2. Standalone or crate module? A loose file with `fn main` we can build with
  //    rustc; anything else we leave for cargo.
  const expanded = fs.readFileSync(filePath, "utf8");
  if (!/\bfn\s+main\s*\(/.test(expanded)) {
    vscode.window.showInformationMessage(
      "Expanded. No `fn main` here — if this file is part of a Cargo crate, run it with cargo."
    );
    return;
  }

  // 3. Compile standalone.
  const out = path.join(
    os.tmpdir(),
    `vbrrun-${Date.now()}${process.platform === "win32" ? ".exe" : ""}`
  );
  const build = await execAsync(
    "rustc",
    ["--edition", "2021", filePath, "-o", out],
    path.dirname(filePath)
  );
  if (build.failed) {
    showErrors("rustc couldn't build this file standalone:", build.stderr || build.stdout);
    vscode.window.showErrorMessage(
      "rustc build failed. If this file belongs to a Cargo project, run it with cargo instead."
    );
    return;
  }

  // 4. Run it.
  runInTerminal(out, [], path.dirname(filePath));
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
    "Bust Language Server",
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
    vscode.commands.registerCommand("vbr.runFile", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) return;
      const lang = editor.document.languageId;
      if (lang === "vbr") {
        if (editor.document.isDirty) await editor.document.save();
        runVbrFile(editor.document.uri);
      } else if (lang === "rust") {
        await embedAndRunRust(editor.document);
      } else {
        vscode.window.showInformationMessage(
          "Open a .vbr file, or a .rs file with an embedded /* vbr … */ block."
        );
      }
    }),
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
