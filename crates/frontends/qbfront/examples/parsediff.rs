//! The HIR the BC-checked front and the grammar-table parser give for the same
//! sources, debug lines included; lists the sources whose HIR differs.
//!
//!     parsediff DIALECT FILE...
use qbfront::semantic::{compile_debugged, DebugSource, Options};
use qbfront::{generated_parser, source, Dialect, Module};

fn hir(
    module: &Module,
    text: &str,
    dialect: Dialect,
    lines: usize,
) -> Result<String, String> {
    let debugged = Some(DebugSource {
        text: text.lines().map(str::to_owned).collect(),
        lines: (1..=lines).collect(),
        debug: true,
    });
    let runtime = qbfront::front::explain::dialect_flag(dialect);
    compile_debugged(module, "m", dialect, runtime, &Options::default(), debugged)
        .map(|(text, warnings)| format!("{text}\n{warnings:?}"))
        .map_err(|error| error.message)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dialect = Dialect::parse(&args[0]).expect("a dialect");
    let (mut same, mut different, mut skipped) = (0, 0, 0);
    for path in &args[1..] {
        let Ok(loaded) = source::load_with_map(std::path::Path::new(path), &[]) else { continue };
        let (Ok(old), Ok(new)) =
            (generated_parser::parse(&loaded.text, dialect), qbfront::front::parse(&loaded.text, dialect))
        else {
            skipped += 1;
            continue;
        };
        let lines = loaded.text.lines().count();
        let (a, b) = (hir(&old, &loaded.text, dialect, lines), hir(&new, &loaded.text, dialect, lines));
        if a == b {
            same += 1;
        } else {
            different += 1;
            let at = match (&a, &b) {
                (Ok(a), Ok(b)) => a.lines().zip(b.lines()).position(|(x, y)| x != y).map_or(0, |n| n + 1),
                _ => 0,
            };
            println!("{path}: HIR differs (first at HIR line {at}; old ok={}, new ok={})", a.is_ok(), b.is_ok());
            if let (Ok(a), Ok(b), true) = (&a, &b, std::env::var_os("SHOW").is_some()) {
                let (x, y) = (
                    a.lines().nth(at.saturating_sub(1)).unwrap_or(""),
                    b.lines().nth(at.saturating_sub(1)).unwrap_or(""),
                );
                let split = x.chars().zip(y.chars()).position(|(p, q)| p != q).unwrap_or(0).saturating_sub(60);
                println!("  old: ...{}", x.chars().skip(split).take(260).collect::<String>());
                println!("  new: ...{}", y.chars().skip(split).take(260).collect::<String>());
            }
            if let (Err(a), Ok(_)) | (Ok(_), Err(a)) = (&a, &b) {
                println!("    {a}");
            }
        }
    }
    eprintln!("same {same}, different {different}, one parser refuses {skipped}");
}
