//! gate DIR|OBJ: raises every object; counts functions verified and refused,
//! and the commonest reasons. One object prints its MIR too.
use std::collections::BTreeMap;
use std::path::PathBuf;

fn shape(reason: &str) -> String {
    let mut out = String::new();
    let mut chars = reason.chars().peekable();
    while let Some(one) = chars.next() {
        if one == '0' && chars.peek() == Some(&'x') {
            chars.next();
            while chars.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                chars.next();
            }
            out.push('#');
        } else {
            out.push(one);
        }
    }
    out
}

fn main() {
    let arg = PathBuf::from(std::env::args().nth(1).expect("a directory or object"));
    let single = arg.is_file();
    let mut paths: Vec<PathBuf> =
        if single { vec![arg] } else { std::fs::read_dir(&arg).unwrap().map(|one| one.unwrap().path()).collect() };
    paths.sort();
    let (mut verified, mut refused, mut broken, mut modules, mut panics) = (0, 0, 0, 0, 0);
    let mut reasons: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut note = |reason: String, example: String| {
        let entry = reasons.entry(shape(&reason)).or_insert((0, example));
        entry.0 += 1;
    };
    for path in paths {
        let Ok(Some(found)) = llrm_omf::module::load(&path) else { continue };
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let raised = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            llrm_x86_bc::raise_each(&found, &llrm_x86_m16::machine::BUILT_IN)
        })) {
            Ok(Ok(raised)) => raised,
            Ok(Err(refusal)) => {
                modules += 1;
                if single {
                    eprintln!("refused module: {}", refusal.reason);
                }
                note(format!("module: {}", refusal.reason), file);
                continue;
            }
            Err(_) => {
                panics += 1;
                eprintln!("panic: {file}");
                continue;
            }
        };
        let errors = llrm_mir::verify::verify(&raised.module);
        if single {
            println!("{}", llrm_mir::print::module(&raised.module));
        }
        for (name, outcome) in &raised.outcomes {
            match outcome {
                Ok(()) => {
                    let prefix = format!("@{name}: ");
                    let mine: Vec<&String> = errors.iter().filter(|one| one.starts_with(&prefix)).collect();
                    if mine.is_empty() {
                        verified += 1;
                    } else {
                        broken += 1;
                        eprintln!("verify {file} {}", mine[0]);
                        note(format!("VERIFY {}", mine[0].trim_start_matches(&prefix)), format!("{file} {name}"));
                    }
                }
                Err(reason) => {
                    refused += 1;
                    if single {
                        eprintln!("refused {name}: {reason}");
                    }
                    note(reason.clone(), format!("{file} {name}"));
                }
            }
        }
        for error in
            errors.iter().filter(|one| !raised.outcomes.iter().any(|(name, _)| one.starts_with(&format!("@{name}: "))))
        {
            eprintln!("verify {file} {error}");
        }
    }
    println!("verified {verified} refused {refused} unverified {broken} modules refused {modules} panics {panics}");
    let mut sorted: Vec<(String, (usize, String))> = reasons.into_iter().collect();
    sorted.sort_by(|a, b| b.1.0.cmp(&a.1.0));
    for (reason, (count, example)) in sorted.iter().take(60) {
        println!("{count:5} {reason}   [{example}]");
    }
}
