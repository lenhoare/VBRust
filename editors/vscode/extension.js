// Minimal VS Code client for the VBR language server.
//
// It launches the `vbr-lsp` binary and connects over stdio. The server does the
// real work (running the VBR compiler and reporting diagnostics); this client is
// just the glue VS Code needs.

const path = require("path");
const fs = require("fs");
const { workspace } = require("vscode");
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

function activate(context) {
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
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
