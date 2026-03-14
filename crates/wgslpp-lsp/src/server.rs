//! LSP protocol handler: dispatches requests and notifications.

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    PublishDiagnostics,
};
use lsp_types::request::{
    Completion, DocumentSymbolRequest, FoldingRangeRequest, Formatting, GotoDefinition,
    HoverRequest, SemanticTokensFullRequest,
};
use lsp_types::{
    CompletionOptions, DocumentSymbolResponse, GotoDefinitionResponse, InitializeParams,
    PublishDiagnosticsParams, SemanticTokensFullOptions, SemanticTokensOptions,
    SemanticTokensServerCapabilities, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri,
};

use crate::completion::completions;
use crate::diagnostics::compute_diagnostics;
use crate::folding::folding_ranges;
use crate::formatting::format_document;
use crate::hover::hover;
use crate::navigation::{find_definition_by_text, goto_definition};
use crate::semantic_tokens::semantic_tokens;
use crate::symbols::document_symbols;
use crate::workspace::Workspace;

/// Run the LSP server on the given connection.
pub fn run(
    connection: &Connection,
    params: InitializeParams,
) -> Result<(), Box<dyn std::error::Error>> {
    #[allow(deprecated)]
    let root = params
        .root_uri
        .and_then(|u| crate::workspace::uri_to_path(u.as_str()))
        .or_else(|| {
            #[allow(deprecated)]
            params.root_path.map(std::path::PathBuf::from)
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let mut workspace = Workspace::new(root);
    workspace.load_config();

    log::info!("wgslpp LSP server started");

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                handle_request(connection, &workspace, req)?;
            }
            Message::Notification(not) => {
                handle_notification(connection, &mut workspace, not)?;
            }
            Message::Response(_) => {}
        }
    }

    Ok(())
}

/// Return server capabilities for the initialize response.
pub fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        document_symbol_provider: Some(lsp_types::OneOf::Left(true)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![
                ".".to_string(),
                "#".to_string(),
                "<".to_string(),
                "\"".to_string(),
            ]),
            ..Default::default()
        }),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: crate::semantic_tokens::legend(),
                full: Some(SemanticTokensFullOptions::Bool(true)),
                range: None,
                ..Default::default()
            },
        )),
        folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
        document_formatting_provider: Some(lsp_types::OneOf::Left(true)),
        ..Default::default()
    }
}

fn handle_request(
    connection: &Connection,
    workspace: &Workspace,
    req: Request,
) -> Result<(), Box<dyn std::error::Error>> {
    let req_id = req.id.clone();

    if let Ok((id, params)) = cast_request::<GotoDefinition>(req.clone()) {
        let uri_str = params
            .text_document_position_params
            .text_document
            .uri
            .as_str()
            .to_string();
        let pos = params.text_document_position_params.position;

        let result = goto_definition(workspace, &uri_str, pos).or_else(|| {
            let source = workspace.documents.get(&uri_str)?;
            let word = word_at(source, pos)?;
            find_definition_by_text(workspace, &uri_str, &word)
        });

        let response = result.map(GotoDefinitionResponse::Scalar);
        send_response(connection, id, response)?;
        return Ok(());
    }

    if let Ok((id, params)) = cast_request::<HoverRequest>(req.clone()) {
        let uri_str = params
            .text_document_position_params
            .text_document
            .uri
            .as_str()
            .to_string();
        let pos = params.text_document_position_params.position;

        let result = hover(workspace, &uri_str, pos);
        send_response(connection, id, result)?;
        return Ok(());
    }

    if let Ok((id, params)) = cast_request::<DocumentSymbolRequest>(req.clone()) {
        let uri_str = params.text_document.uri.as_str().to_string();
        let symbols = document_symbols(workspace, &uri_str);
        let response = DocumentSymbolResponse::Nested(symbols);
        send_response(connection, id, Some(response))?;
        return Ok(());
    }

    if let Ok((id, params)) = cast_request::<Completion>(req.clone()) {
        let uri_str = params
            .text_document_position
            .text_document
            .uri
            .as_str()
            .to_string();
        let pos = params.text_document_position.position;
        let items = completions(workspace, &uri_str, pos);
        let response = lsp_types::CompletionResponse::Array(items);
        send_response(connection, id, Some(response))?;
        return Ok(());
    }

    if let Ok((id, params)) = cast_request::<SemanticTokensFullRequest>(req.clone()) {
        let uri_str = params.text_document.uri.as_str().to_string();
        let result = if let Some(source) = workspace.documents.get(&uri_str) {
            let tokens = semantic_tokens(source);
            Some(lsp_types::SemanticTokensResult::Tokens(
                lsp_types::SemanticTokens {
                    result_id: None,
                    data: tokens,
                },
            ))
        } else {
            None
        };
        send_response(connection, id, result)?;
        return Ok(());
    }

    if let Ok((id, params)) = cast_request::<FoldingRangeRequest>(req.clone()) {
        let uri_str = params.text_document.uri.as_str().to_string();
        let result = if let Some(source) = workspace.documents.get(&uri_str) {
            Some(folding_ranges(source))
        } else {
            None
        };
        send_response(connection, id, result)?;
        return Ok(());
    }

    if let Ok((id, params)) = cast_request::<Formatting>(req.clone()) {
        let uri_str = params.text_document.uri.as_str().to_string();
        let edits = format_document(workspace, &uri_str);
        send_response(connection, id, Some(edits))?;
        return Ok(());
    }

    // Unknown request
    let resp = Response::new_err(
        req_id,
        lsp_server::ErrorCode::MethodNotFound as i32,
        "method not found".to_string(),
    );
    connection.sender.send(Message::Response(resp))?;
    Ok(())
}

fn handle_notification(
    connection: &Connection,
    workspace: &mut Workspace,
    not: Notification,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(params) = cast_notification::<DidOpenTextDocument>(not.clone()) {
        let uri = params.text_document.uri.as_str().to_string();
        workspace.update_document(&uri, params.text_document.text);
        publish_diagnostics(connection, workspace, &uri)?;
        return Ok(());
    }

    if let Ok(params) = cast_notification::<DidChangeTextDocument>(not.clone()) {
        let uri = params.text_document.uri.as_str().to_string();
        if let Some(change) = params.content_changes.into_iter().last() {
            workspace.update_document(&uri, change.text);
            publish_diagnostics(connection, workspace, &uri)?;
        }
        return Ok(());
    }

    if let Ok(params) = cast_notification::<DidSaveTextDocument>(not.clone()) {
        let uri = params.text_document.uri.as_str().to_string();
        if let Some(path) = crate::workspace::uri_to_path(&uri) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                workspace.update_document(&uri, text);
                publish_diagnostics(connection, workspace, &uri)?;
            }
        }
        return Ok(());
    }

    if let Ok(params) = cast_notification::<DidCloseTextDocument>(not.clone()) {
        let uri = params.text_document.uri.as_str().to_string();
        workspace.close_document(&uri);
        return Ok(());
    }

    Ok(())
}

fn publish_diagnostics(
    connection: &Connection,
    workspace: &Workspace,
    uri: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = compute_diagnostics(workspace, uri);

    let parsed_uri: Uri = uri
        .parse()
        .unwrap_or_else(|_| "file:///unknown".parse().unwrap());

    let params = PublishDiagnosticsParams {
        uri: parsed_uri,
        diagnostics,
        version: None,
    };

    let not = lsp_server::Notification::new(
        <PublishDiagnostics as lsp_types::notification::Notification>::METHOD.to_string(),
        params,
    );
    connection.sender.send(Message::Notification(not))?;
    Ok(())
}

fn send_response<T: serde::Serialize>(
    connection: &Connection,
    id: RequestId,
    result: Option<T>,
) -> Result<(), Box<dyn std::error::Error>> {
    let resp = match result {
        Some(r) => Response::new_ok(id, serde_json::to_value(r)?),
        None => Response::new_ok(id, serde_json::Value::Null),
    };
    connection.sender.send(Message::Response(resp))?;
    Ok(())
}

fn cast_request<R: lsp_types::request::Request>(
    req: Request,
) -> Result<(RequestId, R::Params), Request> {
    req.extract(R::METHOD).map_err(|e| match e {
        lsp_server::ExtractError::MethodMismatch(r) => r,
        lsp_server::ExtractError::JsonError { .. } => {
            panic!("JSON deserialization error in LSP request")
        }
    })
}

fn cast_notification<N: lsp_types::notification::Notification>(
    not: Notification,
) -> Result<N::Params, Notification> {
    not.extract(N::METHOD).map_err(|e| match e {
        lsp_server::ExtractError::MethodMismatch(n) => n,
        lsp_server::ExtractError::JsonError { .. } => {
            panic!("JSON deserialization error in LSP notification")
        }
    })
}

fn word_at(source: &str, position: lsp_types::Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let col = position.character as usize;
    if col >= line.len() {
        return None;
    }
    let bytes = line.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut start = col;
    while start > 0 && is_ident(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = col;
    while end < bytes.len() && is_ident(bytes[end]) {
        end += 1;
    }
    if start == end {
        return None;
    }
    Some(line[start..end].to_string())
}
