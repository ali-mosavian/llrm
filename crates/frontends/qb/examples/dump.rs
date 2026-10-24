//! The typed tree of a source, without the checker: `dump DIALECT FILE`.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dialect = llrm_qb::Dialect::parse(&args[0]).expect("a dialect");
    let text = std::fs::read_to_string(&args[1]).expect("a file");
    match llrm_qb::front::ast::parse(&text, dialect) {
        Ok(module) => println!("{:#?}", module.statements),
        Err(error) => println!("{}:{}: {}", error.span.line, error.span.start, error.message),
    }
}
