// Minimal VS Code client for the VBR language server.
//
// It launches the `vbr-lsp` binary and connects over stdio. The server does the
// real work (running the VBR compiler and reporting diagnostics); this client is
// just the glue VS Code needs.

const path = require("path");
const { workspace } = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

function serverCommand(context) {
  // Priority: setting → env var → the debug build alongside this repo.
  const configured = workspace.getConfiguration("vbr").get("serverPath");
  if (configured) return configured;
  if (process.env.VBR_LSP_SERVER) return process.env.VBR_LSP_SERVER;
  // editors/vscode/ → repo root → vbr-lsp/target/release/vbr-lsp
  return context.asAbsolutePath(
    path.join("..", "..", "vbr-lsp", "target", "release", "vbr-lsp")
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
