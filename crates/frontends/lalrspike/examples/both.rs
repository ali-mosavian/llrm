use std::io::Read;
fn strip(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("Span { line: ") {
        out.push_str(&rest[..at]);
        rest = &rest[rest[at..].find('}').map_or(rest.len(), |end| at + end + 1)..];
    }
    out + rest
}
fn main() {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).unwrap();
    for program in text.split("\n\n") {
        let program = format!("{}\n", program.trim_end());
        println!("=== {}", program.trim_end().replace('\n', " / "));
        let d = qbfront::Dialect::QuickBasic45;
        let show = |r: Result<qbfront::syntax::Module, String>| match r {
            Ok(m) => strip(&format!("{:?}", (m.statements, m.procedures))),
            Err(e) => format!("ERR {e}"),
        };
        println!("old {}", show(qbfront::parse(&program, d).map_err(|e| e.message)));
        println!("new {}", show(lalrspike::qb_ast::parse(&program, d).map_err(|e| format!("{:?}", e.span))));
    }
}
