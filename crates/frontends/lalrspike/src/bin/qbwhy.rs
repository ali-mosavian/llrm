//! Why the table-driven parser refuses a file the LALR parser takes.
use qbfront::Dialect;

fn main() {
    for path in std::env::args().skip(1) {
        let source = std::fs::read_to_string(&path).unwrap();
        for dialect in [Dialect::QuickBasic45, Dialect::VbDos, Dialect::Quickr] {
            if let Err(error) = qbfront::parse(&source, dialect) {
                println!("{path} [{dialect:?}]: {error:?}");
                break;
            }
        }
    }
}
