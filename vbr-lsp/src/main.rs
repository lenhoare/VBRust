//! VBR language server.
//!
//! Speaks the Language Server Protocol over stdio and reuses the `vbr` compiler
//! front-end. Tier 1: live diagnostics — on every edit it runs `vbr::compile`
//! and publishes errors/warnings/teaching notes as squiggles, underlining the
//! exact token when the diagnostic carries a span. Tier 2: **hover** (an
//! identifier shows its VB type and the Rust type it lowers to — the resolver
//! records both) and **go-to-definition** (a variable use jumps to its `Dim`).

use std::collections::HashMap;
use std::sync::RwLock;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use vbr::diagnostics::Level;
use vbr::span::LineIndex;

struct Backend {
    client: Client,
    /// The latest text of each open document — hover/definition need to
    /// recompile from it.
    docs: RwLock<HashMap<Url, String>>,
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
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_string()]),
                    ..Default::default()
                }),
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
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.docs.write().unwrap().insert(uri.clone(), text.clone());
        self.validate(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // With FULL sync, the final change carries the whole document.
        if let Some(change) = params.content_changes.into_iter().last() {
            let uri = params.text_document.uri;
            self.docs
                .write()
                .unwrap()
                .insert(uri.clone(), change.text.clone());
            self.validate(uri, &change.text).await;
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let pos = params.text_document_position_params;
        let Some(text) = self.doc(&pos.text_document.uri) else {
            return Ok(None);
        };
        let Some(offset) = byte_offset(&text, pos.position) else {
            return Ok(None);
        };
        let compiled = vbr::compile(&text);
        let index = LineIndex::new(&text);
        // The resolver records one entry per identifier use; pick the
        // narrowest span covering the cursor.
        let hit = compiled
            .hovers
            .iter()
            .filter(|(span, _)| span.contains(offset))
            .min_by_key(|(span, _)| span.end - span.start);
        Ok(hit.map(|(span, display)| Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: display.clone(),
            }),
            range: Some(Range::new(
                lsp_pos(&text, &index, span.start),
                lsp_pos(&text, &index, span.end),
            )),
        }))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let pos = params.text_document_position;
        let Some(text) = self.doc(&pos.text_document.uri) else {
            return Ok(None);
        };
        let Some(offset) = byte_offset(&text, pos.position) else {
            return Ok(None);
        };
        let items: Vec<CompletionItem> = vbr::complete::completions_at(&text, offset)
            .into_iter()
            .map(|c| CompletionItem {
                label: c.label,
                detail: (!c.detail.is_empty()).then_some(c.detail),
                kind: Some(completion_kind(c.kind)),
                ..Default::default()
            })
            .collect();
        if items.is_empty() {
            return Ok(None);
        }
        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let pos = params.text_document_position_params;
        let uri = pos.text_document.uri.clone();
        let Some(text) = self.doc(&uri) else {
            return Ok(None);
        };
        let Some(offset) = byte_offset(&text, pos.position) else {
            return Ok(None);
        };
        let compiled = vbr::compile(&text);
        let index = LineIndex::new(&text);
        let hit = compiled
            .defs
            .iter()
            .find(|(use_span, _)| use_span.contains(offset));
        Ok(hit.map(|(_, decl)| {
            GotoDefinitionResponse::Scalar(Location::new(
                uri,
                Range::new(
                    lsp_pos(&text, &index, decl.start),
                    lsp_pos(&text, &index, decl.end),
                ),
            ))
        }))
    }
}

impl Backend {
    fn doc(&self, uri: &Url) -> Option<String> {
        self.docs.read().unwrap().get(uri).cloned()
    }

    /// Compile `text` and publish its diagnostics for `uri`.
    async fn validate(&self, uri: Url, text: &str) {
        let diagnostics = compile_to_diagnostics(text);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

/// Run the VBR compiler over `text` and translate its structured diagnostics into
/// LSP diagnostics. A diagnostic that carries a byte span underlines exactly that
/// range; one with only a 1-based line spans that whole line.
fn compile_to_diagnostics(text: &str) -> Vec<Diagnostic> {
    let compiled = vbr::compile(text);
    let line_len: Vec<u32> = text.lines().map(|l| l.chars().count() as u32).collect();
    let index = LineIndex::new(text);

    compiled
        .diagnostic_items
        .iter()
        .map(|d| {
            let range = match d.span {
                Some(span) if span.end > span.start => Range::new(
                    lsp_pos(text, &index, span.start),
                    lsp_pos(text, &index, span.end),
                ),
                _ => {
                    // VBR lines are 1-based; a line-less diagnostic is pinned to the top.
                    let line = d.line.map(|l| l.saturating_sub(1) as u32).unwrap_or(0);
                    let end = line_len.get(line as usize).copied().unwrap_or(0).max(1);
                    Range::new(Position::new(line, 0), Position::new(line, end))
                }
            };
            let severity = match d.level {
                Level::Error => DiagnosticSeverity::ERROR,
                Level::Warning => DiagnosticSeverity::WARNING,
                Level::Note => DiagnosticSeverity::INFORMATION,
            };
            Diagnostic {
                range,
                severity: Some(severity),
                source: Some("vbr".to_string()),
                message: d.message.clone(),
                ..Default::default()
            }
        })
        .collect()
}

/// The LSP kind for a VBR completion — drives the icon in the list.
fn completion_kind(kind: vbr::complete::CompletionKind) -> CompletionItemKind {
    use vbr::complete::CompletionKind as K;
    match kind {
        K::Method => CompletionItemKind::METHOD,
        K::Field => CompletionItemKind::FIELD,
        K::Variable => CompletionItemKind::VARIABLE,
        K::Function => CompletionItemKind::FUNCTION,
        K::Constant => CompletionItemKind::CONSTANT,
        K::Namespace => CompletionItemKind::MODULE,
        K::EnumVariant => CompletionItemKind::ENUM_MEMBER,
        K::Enum => CompletionItemKind::ENUM,
        K::Struct => CompletionItemKind::STRUCT,
        K::Keyword => CompletionItemKind::KEYWORD,
    }
}

/// A byte offset as an LSP `Position` — 0-based line plus a UTF-16 column,
/// which is what LSP speaks (`LineIndex` gives byte columns).
fn lsp_pos(text: &str, index: &LineIndex, offset: usize) -> Position {
    let (line, byte_col) = index.position(offset);
    let line_text = text.lines().nth(line).unwrap_or("");
    let col: usize = line_text[..byte_col.min(line_text.len())]
        .encode_utf16()
        .count();
    Position::new(line as u32, col as u32)
}

/// An LSP `Position` (0-based line, UTF-16 column) as a byte offset into `text`.
fn byte_offset(text: &str, pos: Position) -> Option<usize> {
    let mut line_start = 0usize;
    for _ in 0..pos.line {
        line_start += text[line_start..].find('\n')? + 1;
    }
    let line_end = text[line_start..]
        .find('\n')
        .map(|i| line_start + i)
        .unwrap_or(text.len());
    let line_text = &text[line_start..line_end];
    let mut units = 0u32;
    for (byte_in_line, ch) in line_text.char_indices() {
        if units >= pos.character {
            return Some(line_start + byte_in_line);
        }
        units += ch.encode_utf16(&mut [0u16; 2]).len() as u32;
    }
    Some(line_end)
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        docs: RwLock::new(HashMap::new()),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
