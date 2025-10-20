use super::diagnostics;
use super::documents::Documents;
use super::transport::{ReadError, Sender};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Exit, Initialized,
    LogMessage, PublishDiagnostics,
};
use lsp_types::notification::Notification;
use lsp_types::request::{Initialize, Shutdown, Request};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, LogMessageParams, MessageType,
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
            text_document_sync: Some(TextDocumentSyncCapability::Kind(
                TextDocumentSyncKind::FULL,
            )),
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
            self.log_error(format!("received change for unknown document: {}", uri.as_str()));
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
            self.log_error(format!("received close for unknown document: {}", uri.as_str()));
            return;
        }

        self.publish_empty_diagnostics(uri);
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

        if let Err(err) =
            self.transport
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

        if let Err(err) =
            self.transport
                .send_notification(PublishDiagnostics::METHOD, params)
        {
            self.log_error(format!("failed to clear diagnostics: {}", err));
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

        if let Err(_) = self
            .transport
            .send_notification(LogMessage::METHOD, params)
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
