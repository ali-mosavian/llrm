//! Sources the grammar-table parser takes and the BC-checked front refuses, one
//! line each.
//!
//!     newvsold DIALECT FILE...
use qbfront::{generated_parser, source, Dialect};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dialect = Dialect::parse(&args[0]).expect("a dialect");
    let (mut both, mut gap, mut neither, mut leak) = (0, 0, 0, 0);
    for path in &args[1..] {
        let Ok(loaded) = source::load_with_map(std::path::Path::new(path), &[]) else { continue };
        let old = generated_parser::parse(&loaded.text, dialect);
        let new = qbfront::front::parse(&loaded.text, dialect);
        match (old.is_ok(), new) {
            (true, Ok(_)) => both += 1,
            (false, Err(error)) => {
                neither += 1;
                if std::env::var_os("ALL").is_some() {
                    println!("{path}:{}: both refuse: {}", error.span.line, error.message);
                }
            }
            (false, Ok(_)) => leak += 1,
            (true, Err(error)) => {
                gap += 1;
                let location = loaded.location(error.span.line);
                let line = location.map_or(error.span.line, |one| one.line);
                let file = location.map_or(path.clone(), |one| one.path.display().to_string());
                println!("{file}:{line}: {}", error.message);
            }
        }
    }
    eprintln!("both take {both}, only the old {gap}, only the new {leak}, neither {neither}");
}
