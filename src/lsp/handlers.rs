use super::diagnostics;
use super::documents::{DocumentSnapshot, Documents};
use super::transport::{ReadError, Sender};
use crate::ts_type::TsType;
use crate::{compile, parser};
use itertools::Itertools;
use lsp_types::notification::Notification;
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Exit, Initialized,
    LogMessage, PublishDiagnostics,
};
use lsp_types::request::{Initialize, InlayHintRequest, Request, Shutdown};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, InlayHint, InlayHintKind,
    InlayHintLabel, InlayHintOptions, InlayHintParams, InlayHintServerCapabilities,
    InlayHintTooltip, LogMessageParams, MarkupContent, MarkupKind, MessageType, OneOf,
    PublishDiagnosticsParams, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri,
};
use serde_json::Value;
use std::io::Write;

pub enum DispatchAction {
    Continue,
    Exit(i32),
}

pub struct LanguageServer<W: Write> {
    transport: Sender<W>,
    documents: Documents,
    shutdown_requested: bool,
}

impl<W: Write> LanguageServer<W> {
    pub fn new(writer: W) -> Self {
        Self {
            transport: Sender::new(writer),
            documents: Documents::new(),
            shutdown_requested: false,
        }
    }

    pub fn shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }

    pub fn dispatch(&mut self, message: Value) -> DispatchAction {
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").cloned();
        let params = message.get("params").cloned();

        match (method, id) {
            (Some(Initialize::METHOD), Some(id)) => {
                self.handle_initialize(id, params);
                DispatchAction::Continue
            }
            (Some(Shutdown::METHOD), Some(id)) => {
                self.handle_shutdown(id);
                DispatchAction::Continue
            }
            (Some(Exit::METHOD), _) => self.handle_exit(),
            (Some(Initialized::METHOD), _) => {
                self.handle_initialized(params);
                DispatchAction::Continue
            }
            (Some(DidOpenTextDocument::METHOD), _) => {
                self.handle_did_open(params);
                DispatchAction::Continue
            }
            (Some(DidChangeTextDocument::METHOD), _) => {
                self.handle_did_change(params);
                DispatchAction::Continue
            }
            (Some(InlayHintRequest::METHOD), Some(id)) => {
                self.handle_inlay_hint(id, params);
                DispatchAction::Continue
            }
            (Some(DidCloseTextDocument::METHOD), _) => {
                self.handle_did_close(params);
                DispatchAction::Continue
            }
            (Some(_method), Some(id)) => {
                self.respond_method_not_found(id);
                DispatchAction::Continue
            }
            _ => DispatchAction::Continue,
        }
    }

    fn handle_initialize(&mut self, id: Value, params: Option<Value>) {
        let params_value = match params {
            Some(value) => value,
            None => {
                self.send_invalid_params(id, "missing initialize params".to_string());
                return;
            }
        };

        let _params: InitializeParams = match serde_json::from_value(params_value) {
            Ok(params) => params,
            Err(err) => {
                self.send_invalid_params(id, err.to_string());
                return;
            }
        };

        let capabilities = ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
                InlayHintOptions::default(),
            ))),
            ..ServerCapabilities::default()
        };

        let result = InitializeResult {
            capabilities,
            server_info: Some(ServerInfo {
                name: "vegen".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        };

        if let Err(err) = self.transport.send_response(id, result) {
            self.log_error(format!("failed to send initialize response: {}", err));
        }
    }

    fn handle_initialized(&mut self, params: Option<Value>) {
        let _ = params.and_then(|value| serde_json::from_value::<InitializedParams>(value).ok());
    }

    fn handle_shutdown(&mut self, id: Value) {
        self.shutdown_requested = true;
        if let Err(err) = self.transport.send_response(id, Value::Null) {
            self.log_error(format!("failed to send shutdown response: {}", err));
        }
    }

    fn handle_exit(&mut self) -> DispatchAction {
        let code = if self.shutdown_requested { 0 } else { 1 };
        DispatchAction::Exit(code)
    }

    fn handle_did_open(&mut self, params: Option<Value>) {
        let params_value = match params {
            Some(value) => value,
            None => {
                self.log_error("didOpen missing params");
                return;
            }
        };
        let params: DidOpenTextDocumentParams = match serde_json::from_value(params_value) {
            Ok(params) => params,
            Err(err) => {
                self.log_error(format!("invalid didOpen params: {}", err));
                return;
            }
        };

        let uri = params.text_document.uri.clone();
        self.documents.open(params.text_document);
        self.publish_diagnostics_for(&uri);
    }

    fn handle_did_change(&mut self, params: Option<Value>) {
        let params_value = match params {
            Some(value) => value,
            None => {
                self.log_error("didChange missing params");
                return;
            }
        };

        let params: DidChangeTextDocumentParams = match serde_json::from_value(params_value) {
            Ok(params) => params,
            Err(err) => {
                self.log_error(format!("invalid didChange params: {}", err));
                return;
            }
        };

        let uri = params.text_document.uri.clone();
        let version = Some(params.text_document.version);
        if self
            .documents
            .update(&uri, version, &params.content_changes)
            .is_none()
        {
            self.log_error(format!(
                "received change for unknown document: {}",
                uri.as_str()
            ));
            return;
        }

        self.publish_diagnostics_for(&uri);
    }

    fn handle_did_close(&mut self, params: Option<Value>) {
        let params_value = match params {
            Some(value) => value,
            None => {
                self.log_error("didClose missing params");
                return;
            }
        };

        let params: DidCloseTextDocumentParams = match serde_json::from_value(params_value) {
            Ok(params) => params,
            Err(err) => {
                self.log_error(format!("invalid didClose params: {}", err));
                return;
            }
        };

        let uri = params.text_document.uri;
        if self.documents.close(&uri).is_none() {
            self.log_error(format!(
                "received close for unknown document: {}",
                uri.as_str()
            ));
            return;
        }

        self.publish_empty_diagnostics(uri);
    }

    fn handle_inlay_hint(&mut self, id: Value, params: Option<Value>) {
        let params_value = match params {
            Some(value) => value,
            None => {
                self.send_inlay_hints(id, Some(Vec::new()));
                return;
            }
        };

        let params: InlayHintParams = match serde_json::from_value(params_value) {
            Ok(params) => params,
            Err(err) => {
                self.log_error(format!("invalid inlayHint params: {}", err));
                self.send_inlay_hints(id, Some(Vec::new()));
                return;
            }
        };

        let uri = params.text_document.uri;
        let Some(snapshot) = self.documents.snapshot(&uri) else {
            self.send_inlay_hints(id, Some(Vec::new()));
            return;
        };

        let Some(view_types) = self.collect_view_types(&snapshot) else {
            self.send_inlay_hints(id, Some(Vec::new()));
            return;
        };

        let mut hints = Vec::new();
        for info in view_types {
            if info.name_span.context != snapshot.source_id {
                continue;
            }

            let hint = build_inlay_hint(&info, &snapshot);
            hints.push(hint);
        }

        self.send_inlay_hints(id, Some(hints));
    }

    fn collect_view_types(
        &self,
        snapshot: &DocumentSnapshot<'_>,
    ) -> Option<Vec<compile::ViewTypeInfo>> {
        let nodes = parser::parse_template(snapshot.text(), snapshot.source_id).ok()?;
        let output = compile::compile(&nodes).ok()?;
        Some(output.view_types)
    }

    fn publish_diagnostics_for(&mut self, uri: &Uri) {
        let Some(snapshot) = self.documents.snapshot(uri) else {
            self.log_error(format!(
                "attempted to publish diagnostics for unknown document: {}",
                uri.as_str()
            ));
            return;
        };

        let diagnostics = diagnostics::collect(uri, &snapshot);
        let params = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: snapshot.version(),
        };

        if let Err(err) = self
            .transport
            .send_notification(PublishDiagnostics::METHOD, params)
        {
            self.log_error(format!("failed to publish diagnostics: {}", err));
        }
    }

    fn publish_empty_diagnostics(&mut self, uri: Uri) {
        let params = PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics: Vec::new(),
            version: None,
        };

        if let Err(err) = self
            .transport
            .send_notification(PublishDiagnostics::METHOD, params)
        {
            self.log_error(format!("failed to clear diagnostics: {}", err));
        }
    }

    fn send_inlay_hints(&mut self, id: Value, hints: Option<Vec<InlayHint>>) {
        if let Err(err) = self.transport.send_response(id, hints) {
            self.log_error(format!("failed to send inlayHint response: {}", err));
        }
    }

    fn respond_method_not_found(&mut self, id: Value) {
        if let Err(err) = self
            .transport
            .send_error(id, -32601, "method not supported by server")
        {
            self.log_error(format!("failed to send method-not-found response: {}", err));
        }
    }

    fn send_invalid_params(&mut self, id: Value, message: String) {
        if let Err(err) = self.transport.send_error(id, -32602, message) {
            self.log_error(format!("failed to send invalid-params response: {}", err));
        }
    }

    pub fn log_transport_error(&mut self, error: ReadError) {
        if error.is_eof() {
            return;
        }
        self.log_error(error.to_string());
    }

    fn log_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        let params = LogMessageParams {
            typ: MessageType::ERROR,
            message: message.clone(),
        };

        if self
            .transport
            .send_notification(LogMessage::METHOD, params)
            .is_err()
        {
            eprintln!("{}", message);
        }
    }

    pub fn flush(&mut self) {
        if let Err(err) = self.transport.flush() {
            eprintln!("failed to flush transport: {}", err);
        }
    }
}

fn build_inlay_hint(info: &compile::ViewTypeInfo, snapshot: &DocumentSnapshot<'_>) -> InlayHint {
    let range = snapshot.range_from_span(&info.name_span);
    let position = range.end;

    let summary = match top_level_fields(info.input_type.clone()) {
        Some(fields) => fields.iter().map(|(k, _)| k).join(" "),
        None => info.input_type.to_string(),
    };

    let label = InlayHintLabel::from(format!("input=\"{}\"", summary));
    let tooltip = Some(InlayHintTooltip::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: render_type_hover(info),
    }));

    InlayHint {
        position,
        label,
        kind: Some(InlayHintKind::TYPE),
        text_edits: None,
        tooltip,
        padding_left: Some(true),
        padding_right: None,
        data: None,
    }
}

fn render_type_hover(info: &compile::ViewTypeInfo) -> String {
    let formatted = format_ts_type_pretty(&info.input_type, 0);
    format!("```ts\ntype {}Input = {};\n```", info.name, formatted)
}

fn top_level_fields(ty: TsType) -> Option<Vec<(String, TsType)>> {
    match ty {
        TsType::Object(fields) | TsType::View(fields) => Some(fields.into_iter().collect()),
        _ => None,
    }
}

fn format_ts_type_pretty(ty: &TsType, indent: usize) -> String {
    match ty {
        TsType::SimpleType(s) => s.clone(),
        TsType::Array(elem) => format!("{}[]", format_ts_type_pretty(elem, indent)),
        TsType::Function(params, ret) => {
            let params_rendered: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, p)| format!("arg{}: {}", i, format_ts_type_pretty(p, indent)))
                .collect();
            format!(
                "({}) => {}",
                params_rendered.join(", "),
                format_ts_type_pretty(ret, indent)
            )
        }
        TsType::Object(fields) => format_object(fields, indent),
        TsType::View(fields) => {
            let inner = format_object(fields, indent);
            format!("View<{}>", inner)
        }
        TsType::Union(types) => format_union(types, indent),
    }
}

fn format_object(fields: &std::collections::BTreeMap<String, TsType>, indent: usize) -> String {
    if fields.is_empty() {
        return "{}".to_string();
    }

    let indent_str = "  ".repeat(indent);
    let inner_indent = "  ".repeat(indent + 1);
    let mut lines = Vec::new();
    for (name, ty) in fields {
        let rendered = format_ts_type_pretty(ty, indent + 1);
        lines.push(format!("{}{}: {};", inner_indent, name, rendered));
    }

    format!("{{\n{}\n{}}}", lines.join("\n"), indent_str)
}

fn format_union(types: &[TsType], indent: usize) -> String {
    if types.is_empty() {
        return "never".to_string();
    }

    let indent_str = "  ".repeat(indent);
    let mut iter = types.iter();
    let first = format_ts_type_pretty(iter.next().unwrap(), indent);
    let mut result = first;
    for ty in iter {
        let rendered = format_ts_type_pretty(ty, indent);
        result.push_str(&format!("\n{}| {}", indent_str, rendered));
    }
    result
}
