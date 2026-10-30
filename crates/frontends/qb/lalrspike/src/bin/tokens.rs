fn main() {
    let source = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    for token in llrm_nib::lexer::lex(&source).unwrap() {
        println!("{}:{} {:?}", token.span.line, token.span.column, token.kind);
    }
}
