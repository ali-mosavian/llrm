//! Nib's language server, over stdio: the frontend's diagnostics, and the
//! symbols, definitions, hovers and completions its syntax and checker give.

mod completion;
mod definition;
mod diagnostics;
mod documents;
mod hover;
mod index;
mod protocol;
mod resolve;
mod symbols;
mod text;
mod transport;

#[cfg(test)]
mod tests;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use documents::Documents;
use protocol::{At, Changed, Document, Opened, Saved};

/// Answers the messages on `input` until `exit` or its end.
pub fn serve(mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut server = Server::default();
    while let Some(message) = transport::read(&mut input)? {
        for one in server.handle(&message) {
            transport::write(&mut output, &one)?;
        }
        if server.exited {
            break;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Server {
    documents: Documents,
    published: diagnostics::Published,
    exited: bool,
}

const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

impl Server {
    /// The messages `message` causes: its reply, when it is a request, and notifications.
    fn handle(&mut self, message: &Value) -> Vec<Value> {
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match message.get("method").and_then(Value::as_str).unwrap_or("") {
            "initialize" => reply(id, Ok(capabilities())),
            "shutdown" => reply(id, Ok(Value::Null)),
            "exit" => {
                self.exited = true;
                Vec::new()
            }
            "textDocument/didOpen" => self.edited(params, |one: Opened| (one.text_document.uri, Some(one.text_document.text))),
            "textDocument/didChange" => self.edited(params, |one: Changed| (one.text_document.uri, one.content_changes.into_iter().last().map(|change| change.text))),
            "textDocument/didSave" => self.edited(params, |one: Saved| (one.text_document.uri, one.text)),
            "textDocument/didClose" => {
                let Some(path) = parsed::<Document>(params).ok().and_then(|one| documents::path(&one.text_document.uri)) else {
                    return Vec::new();
                };
                self.documents.close(&path);
                publish(self.published.replace(&path, Vec::new()))
            }
            "textDocument/documentSymbol" => reply(id, answer(params, |one: Document| {
                documents::path(&one.text_document.uri).map(|path| symbols::symbols(&self.documents, &path))
            })),
            "textDocument/definition" => reply(id, self.at(params, definition::definition)),
            "textDocument/hover" => reply(id, self.at(params, hover::hover)),
            "textDocument/completion" => reply(id, self.at(params, |documents, path, position| Some(completion::completion(documents, path, position)))),
            method if id.is_some() => reply(id, Err((METHOD_NOT_FOUND, format!("no method {method}")))),
            _ => Vec::new(),
        }
    }

    /// Takes a document's new text, if any, and checks every open document again.
    fn edited<P: DeserializeOwned>(&mut self, params: Value, edit: impl FnOnce(P) -> (String, Option<String>)) -> Vec<Value> {
        let Ok((uri, text)) = parsed(params).map(edit) else {
            return Vec::new();
        };
        let Some(path) = documents::path(&uri) else {
            return Vec::new();
        };
        if let Some(text) = text {
            self.documents.open(path, text);
        }
        let mut files = Vec::new();
        for path in self.documents.paths() {
            let found = diagnostics::checked(&mut self.documents, &path);
            files.extend(self.published.replace(&path, found));
        }
        publish(files)
    }

    /// Answers a request about a position in a document.
    fn at<R: Serialize>(
        &self,
        params: Value,
        answer_at: impl FnOnce(&Documents, &std::path::Path, protocol::Position) -> Option<R>,
    ) -> Result<Value, (i64, String)> {
        answer(params, |one: At| answer_at(&self.documents, &documents::path(&one.text_document.uri)?, one.position))
    }
}

fn parsed<P: DeserializeOwned>(params: Value) -> Result<P, (i64, String)> {
    serde_json::from_value(params).map_err(|error| (INVALID_PARAMS, error.to_string()))
}

/// `respond`'s answer to `params`: `null` for none.
fn answer<P: DeserializeOwned, R: Serialize>(params: Value, respond: impl FnOnce(P) -> Option<R>) -> Result<Value, (i64, String)> {
    Ok(serde_json::to_value(respond(parsed(params)?)).expect("serializes"))
}

fn reply(id: Option<Value>, result: Result<Value, (i64, String)>) -> Vec<Value> {
    let Some(id) = id else {
        return Vec::new();
    };
    vec![match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
    }]
}

fn publish(files: Vec<(PathBuf, Vec<protocol::Diagnostic>)>) -> Vec<Value> {
    files
        .into_iter()
        .map(|(file, diagnostics)| {
            json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": {"uri": documents::uri(&file), "diagnostics": diagnostics},
            })
        })
        .collect()
}

fn capabilities() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": {"openClose": true, "change": 1, "save": {"includeText": true}},
            "documentSymbolProvider": true,
            "definitionProvider": true,
            "hoverProvider": true,
            "completionProvider": {"triggerCharacters": ["."]},
        },
        "serverInfo": {"name": "nib-lsp"},
    })
}
