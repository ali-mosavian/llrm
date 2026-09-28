//! Generates instruction selection from `patterns.isel` and `x86.instr`.

// The tests read what build.rs does not.
#[allow(dead_code)]
#[path = "src/backend/isel/generator/mod.rs"]
mod generator;

fn main() {
    let patterns = "src/backend/isel/patterns.isel";
    let forms = "../../target/llrm-x86-code16/src/instructions/x86.instr";
    for path in [patterns, forms, "src/backend/isel/generator"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
    let generated = generator::generate(&read(forms), &read(patterns)).unwrap_or_else(|error| panic!("{error}"));
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("isel.rs");
    std::fs::write(out, generated.code).unwrap();
}
