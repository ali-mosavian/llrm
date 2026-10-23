//! Print the existing parser's Module for each line on stdin (blank line ends a program).
use std::io::Read;
fn main() {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).unwrap();
    for program in text.split("\n\n") {
        let program = format!("{}\n", program.trim_end());
        println!("=== {}", program.trim_end().replace('\n', " / "));
        match qbfront::parse(&program, qbfront::Dialect::QuickBasic45) {
            Ok(module) => {
                for s in &module.statements { println!("{s:?}"); }
                for p in &module.procedures { println!("{p:?}"); }
            }
            Err(e) => println!("ERR {} at {:?}", e.message, e.span),
        }
    }
}
