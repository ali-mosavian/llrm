//! The HIR of each source, debug lines included, written to a directory:
//! `hirdump DIALECT OUTDIR FILE...`. Two runs from different builds, diffed,
//! show what a refactor changed.
use llrm_qb::semantic::{DebugSource, Options, compile_debugged};
use llrm_qb::{Dialect, source};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dialect = Dialect::parse(&args[0]).expect("a dialect");
    let out = std::path::Path::new(&args[1]);
    std::fs::create_dir_all(out).unwrap();
    for path in &args[2..] {
        let Ok(loaded) = source::load_with_map(std::path::Path::new(path), &[]) else { continue };
        let text = match llrm_qb::front::parse(&loaded.text, dialect) {
            Err(error) => format!("refused: {}:{} {}", error.span.line, error.span.start, error.code),
            Ok(module) => {
                let lines = loaded.text.lines().count();
                let debugged = Some(DebugSource {
                    text: loaded.text.lines().map(str::to_owned).collect(),
                    lines: (1..=lines).collect(),
                    debug: true,
                });
                let runtime = llrm_qb::front::explain::dialect_flag(dialect);
                match compile_debugged(&module, "m", dialect, runtime, &Options::default(), debugged) {
                    Ok((hir, warnings)) => format!("{hir}\n{warnings:?}"),
                    Err(error) => format!("compile error: {}", error.message),
                }
            }
        };
        std::fs::write(out.join(path.replace('/', "_")), text).unwrap();
    }
}
