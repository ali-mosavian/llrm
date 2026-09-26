//! The server driven in process, as an editor drives it.

use std::io::Cursor;

use serde_json::{Value, json};

use super::{documents, serve, transport};

const GEO: &str = "\
# A point on the plane.
pub struct Point:
    mut x: i16
    y: i16

# The squared length.
pub fn Point.length2(self: &Point) -> i16:
    return self.x * self.x + self.y * self.y

pub fn origin() -> Point:
    return Point(x=0, y=0)

fn secret() -> i16:
    return 7
";

const MAIN: &str = "\
import geo
import std.io as io

# Twice `value`.
fn double(value: i16) -> i16:
    return value * 2

fn main() -> i16:
    let p = geo.Point(x=3, y=4)
    let total = double(p.x)
    let mode = io.READ
    print(total)
    return p.length2()
";

/// `main.nib` and `geo.nib` on disk, and the messages opening `main` in an editor.
struct Program {
    root: tempfile::TempDir,
}

impl Program {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("a temporary directory");
        for (name, text) in [("main", MAIN), ("geo", GEO)] {
            std::fs::write(root.path().join(name).with_extension(crate::modules::EXTENSION), text).expect("written");
        }
        Self { root }
    }

    fn uri(&self, module: &str) -> String {
        documents::uri(&crate::modules::file(self.root.path(), module))
    }

    fn opened(&self, module: &str, text: &str) -> Value {
        notification("textDocument/didOpen", json!({"textDocument": {"uri": self.uri(module), "text": text}}))
    }

    fn at(&self, line: u32, character: u32) -> Value {
        json!({"textDocument": {"uri": self.uri("main")}, "position": {"line": line, "character": character}})
    }
}

fn request(id: u32, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

fn notification(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

/// Every message the server sends for `messages`, framed as over stdio.
fn session(messages: &[Value]) -> Vec<Value> {
    let mut input = Vec::new();
    for one in [&[request(0, "initialize", json!({}))], messages].concat() {
        transport::write(&mut input, &one).expect("framed");
    }
    let mut output = Vec::new();
    serve(Cursor::new(input), &mut output).expect("served");
    let mut output = Cursor::new(output);
    std::iter::from_fn(|| transport::read(&mut output).expect("framed")).collect()
}

fn result(sent: &[Value], id: u32) -> &Value {
    &sent.iter().find(|one| one["id"] == id).expect("a reply")["result"]
}

/// The diagnostics last published for `uri`.
fn published<'s>(sent: &'s [Value], uri: &str) -> &'s Vec<Value> {
    let last = sent.iter().rev().find(|one| one["params"]["uri"] == uri).expect("published");
    last["params"]["diagnostics"].as_array().expect("diagnostics")
}

#[test]
fn diagnostics_follow_the_error_and_clear_when_it_is_fixed() {
    let program = Program::new();
    let broken = session(&[program.opened("main", &MAIN.replace("double(p.x)", "double(true)"))]);
    assert_eq!(broken[0]["result"]["serverInfo"]["name"], "nib-lsp");
    let diagnostics = published(&broken, &program.uri("main"));
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["message"], "expected i16, found bool");
    assert_eq!(diagnostics[0]["range"]["start"], json!({"line": 9, "character": 23}));

    let changed = json!({"textDocument": {"uri": program.uri("main")}, "contentChanges": [{"text": MAIN}]});
    let fixed = session(&[
        program.opened("main", &MAIN.replace("double(p.x)", "double(true)")),
        notification("textDocument/didChange", changed),
    ]);
    assert!(published(&fixed, &program.uri("main")).is_empty());
}

#[test]
fn an_imported_module_error_is_on_its_file_and_on_the_import() {
    let program = Program::new();
    let sent = session(&[program.opened("main", MAIN), program.opened("geo", &GEO.replace("return 7", "return true"))]);
    let in_geo = published(&sent, &program.uri("geo"));
    assert_eq!(in_geo.len(), 1);
    assert_eq!(in_geo[0]["range"]["start"]["line"], 13);
    let in_main = published(&sent, &program.uri("main"));
    assert_eq!(in_main.len(), 1);
    assert_eq!(in_main[0]["message"], "geo: expected i16, found bool");
    assert_eq!(in_main[0]["range"]["start"]["line"], 0);
}

#[test]
fn symbols_are_the_declarations_with_their_members() {
    let program = Program::new();
    let sent = session(&[program.opened("geo", GEO), request(1, "textDocument/documentSymbol", json!({"textDocument": {"uri": program.uri("geo")}}))]);
    let symbols = result(&sent, 1).as_array().expect("symbols");
    let names: Vec<&str> = symbols.iter().map(|one| one["name"].as_str().expect("named")).collect();
    assert_eq!(names, ["Point", "Point.length2", "origin", "secret"]);
    assert_eq!(symbols[0]["children"].as_array().expect("fields").iter().map(|one| &one["name"]).collect::<Vec<_>>(), ["x", "y"]);
    assert_eq!(symbols[0]["selectionRange"]["start"], json!({"line": 1, "character": 11}));
}

#[test]
fn definition_finds_locals_members_imports_and_std() {
    let program = Program::new();
    let positions = [(9, 23), (9, 25), (12, 14), (8, 16), (9, 17), (10, 18), (1, 12)];
    let requests = positions.iter().enumerate().map(|(id, &(line, character))| request(id as u32 + 1, "textDocument/definition", program.at(line, character)));
    let sent = session(&[vec![program.opened("main", MAIN)], requests.collect()].concat());
    let found = |id| {
        let one = result(&sent, id);
        (one["uri"].as_str().expect("a location").to_owned(), one["range"]["start"]["line"].as_u64().expect("a line"))
    };
    assert_eq!(found(1), (program.uri("main"), 8), "the local p");
    assert_eq!(found(2), (program.uri("geo"), 2), "the field x, by p's type");
    assert_eq!(found(3), (program.uri("geo"), 6), "the method length2, by p's type");
    assert_eq!(found(4), (program.uri("geo"), 1), "geo.Point");
    assert_eq!(found(5), (program.uri("main"), 4), "double");
    let (std_uri, line) = found(6);
    assert!(std_uri.contains("nib-lsp/std/io"), "{std_uri}");
    assert_eq!(line, 11, "io.READ");
    assert!(found(7).0.contains("nib-lsp/std/io"), "the import of std.io");
}

#[test]
fn hover_shows_a_signature_with_its_comment_and_a_local_type() {
    let program = Program::new();
    let sent = session(&[
        program.opened("main", MAIN),
        request(1, "textDocument/hover", program.at(9, 17)),
        request(2, "textDocument/hover", program.at(11, 11)),
        request(3, "textDocument/hover", program.at(8, 8)),
    ]);
    let hover = |id| result(&sent, id)["contents"]["value"].as_str().expect("markdown").to_owned();
    assert_eq!(hover(1), "```nib\nfn double(value: i16) -> i16\n```\n\nTwice `value`.");
    assert_eq!(hover(2), "```nib\ntotal: i16\n```");
    assert_eq!(hover(3), "```nib\np: geo.Point\n```", "where p is bound");
}

#[test]
fn completion_offers_keywords_own_names_and_a_modules_public_ones() {
    let program = Program::new();
    let typing = MAIN.replace("    print(total)\n", "    print(total)\n    geo.\n");
    let changed = json!({"textDocument": {"uri": program.uri("main")}, "contentChanges": [{"text": typing}]});
    let sent = session(&[
        program.opened("main", MAIN),
        request(1, "textDocument/completion", program.at(5, 4)),
        notification("textDocument/didChange", changed),
        request(2, "textDocument/completion", program.at(12, 8)),
    ]);
    let labels = |id| result(&sent, id).as_array().expect("items").iter().map(|one| one["label"].as_str().expect("a label").to_owned()).collect::<Vec<_>>();
    let anywhere = labels(1);
    for expected in ["let", "fn", "double", "main", "geo"] {
        assert!(anywhere.iter().any(|one| one == expected), "{expected} in {anywhere:?}");
    }
    assert_eq!(labels(2), ["Point", "origin"]);
}

/// Only `alias.` was completed: `p.` and `ps[1].le` offered nothing.
#[test]
fn completion_after_a_value_offers_its_types_fields_and_methods() {
    let program = Program::new();
    let typing = |member: &str| {
        let text = MAIN.replace("    print(total)\n", &format!("    print(total)\n    let ps: geo.Point[2] = [p, p]\n    {member}\n"));
        json!({"textDocument": {"uri": program.uri("main")}, "contentChanges": [{"text": text}]})
    };
    let sent = session(&[
        program.opened("main", MAIN),
        notification("textDocument/didChange", typing("p.")),
        request(1, "textDocument/completion", program.at(13, 6)),
        notification("textDocument/didChange", typing("ps[1].le")),
        request(2, "textDocument/completion", program.at(13, 12)),
    ]);
    let labels = |id| result(&sent, id).as_array().expect("items").iter().map(|one| one["label"].as_str().expect("a label").to_owned()).collect::<Vec<_>>();
    assert_eq!(labels(1), ["x", "y", "length2"]);
    assert_eq!(labels(2), ["x", "y", "length2"]);
}

/// Locals were never offered: `to` in `main` did not complete `total`.
#[test]
fn completion_offers_the_locals_in_scope_at_the_cursor() {
    let program = Program::new();
    let typing = MAIN.replace("    let mode = io.READ\n", "    to\n    let mode = io.READ\n");
    let changed = json!({"textDocument": {"uri": program.uri("main")}, "contentChanges": [{"text": typing}]});
    let sent = session(&[program.opened("main", MAIN), notification("textDocument/didChange", changed), request(1, "textDocument/completion", program.at(10, 6))]);
    let labels: Vec<String> = result(&sent, 1).as_array().expect("items").iter().map(|one| one["label"].as_str().expect("a label").to_owned()).collect();
    for expected in ["total", "p"] {
        assert!(labels.iter().any(|one| one == expected), "{expected} in {labels:?}");
    }
    for hidden in ["value", "mode"] {
        assert!(!labels.iter().any(|one| one == hidden), "{hidden}, out of scope, in {labels:?}");
    }
}
