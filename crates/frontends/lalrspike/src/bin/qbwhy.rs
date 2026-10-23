//! Why the table-driven parser refuses a file the LALR parser takes.
use qbfront::Dialect;

fn main() {
    for path in std::env::args().skip(1) {
        let source = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        for dialect in [Dialect::QuickBasic45, Dialect::VbDos, Dialect::Quickr] {
            if let Err(error) = qbfront::parse(&source, dialect) {
                println!("{path} [{dialect:?}]: {error:?}");
                break;
            }
        }
    }
}
