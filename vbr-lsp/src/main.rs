//! VBR language server (proof of concept).
//!
//! Speaks the Language Server Protocol over stdio and reuses the `vbr` compiler
//! front-end to give **live diagnostics** in the editor: on every edit it runs
//! `vbr::compile` and publishes the errors, warnings, and teaching notes as
//! squiggles. This is tier 1 — line-level diagnostics. Hover, completion, and
//! go-to-definition come later, once the compiler tracks column spans.

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use vbr::diagnostics::Level;

struct Backend {
    client: Client,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // We recompile the whole document on each change (FULL sync).
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "vbr-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "vbr-lsp ready")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.validate(params.text_document.uri, &params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // With FULL sync, the final change carries the whole document.
        if let Some(change) = params.content_changes.into_iter().last() {
            self.validate(params.text_document.uri, &change.text).await;
        }
    }
}

impl Backend {
    /// Compile `text` and publish its diagnostics for `uri`.
    async fn validate(&self, uri: Url, text: &str) {
        let diagnostics = compile_to_diagnostics(text);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

/// Run the VBR compiler over `text` and translate its structured diagnostics into
/// LSP diagnostics. VBR reports a 1-based line (and no column yet), so each is
/// shown spanning that whole line — good enough for a proof of concept.
fn compile_to_diagnostics(text: &str) -> Vec<Diagnostic> {
    let compiled = vbr::compile(text);
    let line_len: Vec<u32> = text.lines().map(|l| l.chars().count() as u32).collect();

    compiled
        .diagnostic_items
        .iter()
        .map(|d| {
            // VBR lines are 1-based; a line-less diagnostic is pinned to the top.
            let line = d.line.map(|l| l.saturating_sub(1) as u32).unwrap_or(0);
            let end = line_len.get(line as usize).copied().unwrap_or(0).max(1);
            let severity = match d.level {
                Level::Error => DiagnosticSeverity::ERROR,
                Level::Warning => DiagnosticSeverity::WARNING,
                Level::Note => DiagnosticSeverity::INFORMATION,
            };
            Diagnostic {
                range: Range::new(Position::new(line, 0), Position::new(line, end)),
                severity: Some(severity),
                source: Some("vbr".to_string()),
                message: d.message.clone(),
                ..Default::default()
            }
        })
        .collect()
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend { client });
    Server::new(stdin, stdout, socket).serve(service).await;
}
