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

// ── Rust ↔ VBR line mapping (slice 3: step-sync + jump-back) ────────────────

// The sidecar `vbr debugbuild` writes next to a generated <stem>.rs.
// { source: "<abs .vbr path>", map: [[rustLine, vbrLine], ...] }
function loadLineMap(rustFsPath) {
  const mapPath = rustFsPath.replace(/\.rs$/i, ".linemap.json");
  try {
    return JSON.parse(fs.readFileSync(mapPath, "utf8"));
  } catch (_) {
    return null;
  }
}

// The VBR line a Rust line came from: the last checkpoint at or before it
// (mirrors main.rs::vbr_line_for; checkpoints are in ascending Rust order).
function vbrLineFor(map, rustLine) {
  let vbr = null;
  for (const [r, v] of map) {
    if (r <= rustLine) vbr = v;
    else break;
  }
  return vbr;
}

// Resolve the .vbr source behind whatever editor is focused — the real file, a
// generated .vbrdebug/<stem>.rs (via its line map), or the live vbr-rust view.
// This is what lets Debug work no matter which pane has focus.
function resolveVbrSource(uri) {
  if (!uri) return null;
  if (uri.scheme === "file" && /\.vbr$/i.test(uri.fsPath)) return uri.fsPath;
  if (uri.scheme === "vbr-rust") {
    return uri.with({ scheme: "file", path: uri.path.replace(/\.rs$/i, "") }).fsPath;
  }
  if (uri.scheme === "file" && /\.rs$/i.test(uri.fsPath) && /\.vbrdebug[\\/]/i.test(uri.fsPath)) {
    const lm = loadLineMap(uri.fsPath);
    if (lm && lm.source) return lm.source;
  }
  return null;
}

// Run `vbr debugbuild <src>` and resolve to the binary path it prints (the last
// non-empty stdout line). Rejects with the transpiler diagnostics on failure.
function debugBuild(sourcePath) {
  return new Promise((resolve, reject) => {
    const root = rootFor(sourcePath);
    const bin = vbrBinary(root);
    let cmd, args;
    if (bin) {
      cmd = bin;
      args = ["debugbuild", sourcePath];
    } else {
      cmd = process.platform === "win32" ? "cargo.exe" : "cargo";
      args = [
        "run", "--quiet",
        "--manifest-path", path.join(root, "Cargo.toml"),
        "--", "debugbuild", sourcePath,
      ];
    }
    execFile(cmd, args, { cwd: root, maxBuffer: 16 * 1024 * 1024 }, (err, stdout, stderr) => {
      if (err) {
        reject(new Error((stderr || err.message || "").trim()));
        return;
      }
      const lines = stdout.split(/\r?\n/).filter((l) => l.trim());
      const last = lines[lines.length - 1];
      if (!last) {
        reject(new Error("debugbuild produced no binary path"));
        return;
      }
      resolve(last.trim());
    });
  });
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

  // 3. Step-sync: highlight the .vbr line as you step the generated Rust, plus
  //    a jump-back command from a Rust line to its VB line.
  const stepDecoration = vscode.window.createTextEditorDecorationType({
    isWholeLine: true,
    backgroundColor: new vscode.ThemeColor("editor.stackFrameHighlightBackground"),
    overviewRulerColor: new vscode.ThemeColor("editor.stackFrameHighlightBackground"),
    overviewRulerLane: vscode.OverviewRulerLane.Full,
  });

  function clearStepHighlight() {
    for (const ed of vscode.window.visibleTextEditors) {
      ed.setDecorations(stepDecoration, []);
    }
  }

  // Show + highlight a .vbr line. Reuses an already-visible editor for that file
  // (so we don't fight the debugger over columns); otherwise opens it beside.
  async function highlightVbrLine(sourcePath, vbrLine1) {
    const uri = vscode.Uri.file(sourcePath);
    let editor = vscode.window.visibleTextEditors.find(
      (e) => e.document.uri.fsPath === uri.fsPath
    );
    if (!editor) {
      const doc = await vscode.workspace.openTextDocument(uri);
      editor = await vscode.window.showTextDocument(doc, {
        viewColumn: vscode.ViewColumn.Beside,
        preserveFocus: true,
        preview: false,
      });
    }
    const line = Math.max(0, vbrLine1 - 1);
    const range = new vscode.Range(line, 0, line, 0);
    editor.setDecorations(stepDecoration, [range]);
    editor.revealRange(range, vscode.TextEditorRevealType.InCenterIfOutsideViewport);
  }

  // On each stop, find the topmost frame in a generated <stem>.rs (one with a
  // sibling line map) and light up the VB line it came from.
  function handleStop(frames) {
    for (const f of frames || []) {
      const p = f.source && f.source.path;
      if (!p || !/\.rs$/i.test(p)) continue;
      const lm = loadLineMap(p);
      if (!lm) continue;
      const vbrLine = vbrLineFor(lm.map, f.line);
      if (vbrLine != null) highlightVbrLine(lm.source, vbrLine);
      return;
    }
  }

  // Resolve the focused editor back to its .vbr, build a debuggable binary, and
  // open the generated Rust beside it (breakpoints bind to the Rust, not the
  // .vbr). Returns { src, binPath } or null. No dependence on ${file} variables,
  // so it's not fooled by the Rust view or the generated .rs being focused.
  async function buildActiveTarget() {
    const ed = vscode.window.activeTextEditor;
    const src = ed ? resolveVbrSource(ed.document.uri) : null;
    if (!src) {
      vscode.window.showErrorMessage("Open a .vbr file to debug.");
      return null;
    }
    let binPath;
    try {
      binPath = await vscode.window.withProgress(
        { location: vscode.ProgressLocation.Window, title: "VBR: building debug binary…" },
        () => debugBuild(src)
      );
    } catch (e) {
      const out = vscode.window.createOutputChannel("VBR Debug Build");
      out.append(e.message || String(e));
      out.show(true);
      vscode.window.showErrorMessage("VBR debug build failed — see the 'VBR Debug Build' output.");
      return null;
    }
    const rsPath = binPath.replace(/\.exe$/i, "") + ".rs";
    try {
      const rsDoc = await vscode.workspace.openTextDocument(vscode.Uri.file(rsPath));
      await vscode.window.showTextDocument(rsDoc, {
        viewColumn: vscode.ViewColumn.Beside,
        preserveFocus: true,
        preview: false,
      });
    } catch (_) {
      /* non-fatal — still launch */
    }
    return { src, binPath };
  }

  // The `${command:vbr.debugTargetPath}` used by launch.json's `program`: build
  // and return the binary path (so the Run panel + default F5 work, focus-proof).
  async function debugTargetPath() {
    const t = await buildActiveTarget();
    if (!t) throw new Error("VBR debug build failed");
    return t.binPath;
  }

  // The 🐞 button / palette entry — self-contained (doesn't need launch.json).
  async function debugCurrent() {
    const t = await buildActiveTarget();
    if (!t) return;
    const folder = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(t.src));
    await vscode.debug.startDebugging(folder, {
      type: "lldb",
      request: "launch",
      name: "VBR: Debug " + path.basename(t.src),
      program: t.binPath,
      cwd: path.dirname(t.src),
      sourceLanguages: ["rust"],
    });
  }

  async function revealSourceLine() {
    const editor = vscode.window.activeTextEditor;
    if (!editor || !/\.rs$/i.test(editor.document.uri.fsPath)) {
      vscode.window.showInformationMessage("Open a generated Rust file (.vbrdebug/…) first.");
      return;
    }
    const lm = loadLineMap(editor.document.uri.fsPath);
    if (!lm) {
      vscode.window.showInformationMessage("No line map beside this file — debug the .vbr (F5) to generate one.");
      return;
    }
    const vbrLine = vbrLineFor(lm.map, editor.selection.active.line + 1);
    if (vbrLine == null) {
      vscode.window.showInformationMessage("No VB line maps to here.");
      return;
    }
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(lm.source));
    const ed = await vscode.window.showTextDocument(doc, {
      viewColumn: vscode.ViewColumn.One,
      preview: false,
    });
    const pos = new vscode.Position(Math.max(0, vbrLine - 1), 0);
    ed.selection = new vscode.Selection(pos, pos);
    ed.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
  }

  context.subscriptions.push(
    vscode.workspace.registerTextDocumentContentProvider("vbr-rust", rustProvider),
    vscode.commands.registerCommand("vbr.showRustOutput", showRustOutput),
    vscode.commands.registerCommand("vbr.debug", debugCurrent),
    vscode.commands.registerCommand("vbr.debugTargetPath", debugTargetPath),
    vscode.commands.registerCommand("vbr.revealSourceLine", revealSourceLine),
    vscode.workspace.onDidChangeTextDocument((e) => {
      if (e.document.languageId === "vbr") scheduleRefresh(e.document.uri);
    }),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === "vbr") rustProvider.refresh(rustUriFor(doc.uri));
    }),
    stepDecoration,
    vscode.debug.registerDebugAdapterTrackerFactory("lldb", {
      createDebugAdapterTracker() {
        return {
          onDidSendMessage(m) {
            if (
              m.type === "response" &&
              m.command === "stackTrace" &&
              m.body &&
              m.body.stackFrames
            ) {
              handleStop(m.body.stackFrames);
            }
          },
        };
      },
    }),
    vscode.debug.onDidTerminateDebugSession(clearStepHighlight)
  );
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
