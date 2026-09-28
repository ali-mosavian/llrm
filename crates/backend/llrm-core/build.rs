//! Generates instruction selection from `patterns.isel` and the peephole's
//! matchers from `peephole.peep`, both against `x86.instr`.

// The tests read what build.rs does not.
#[allow(dead_code)]
#[path = "src/backend/isel/generator/mod.rs"]
mod generator;

fn main() {
    let patterns = "src/backend/isel/patterns.isel";
    let rules = "src/backend/peephole.peep";
    let forms = "../../target/llrm-x86-code16/src/instructions/x86.instr";
    for path in [patterns, rules, forms, "src/backend/isel/generator"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let generated = generator::generate(&read(forms), &read(patterns)).unwrap_or_else(|error| panic!("{error}"));
    std::fs::write(out.join("isel.rs"), generated.code).unwrap();
    let made = match llrm_peepgen::generate(&read(forms), "x86.instr", &read(rules), "peephole.peep") {
        Ok(made) => made,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    std::fs::write(out.join("peephole.rs"), made.rules).unwrap();
}
