//! Generates one instruction selector per target from its definition
//! directory (`crates/target/<name>/src/isel/patterns.isel` against
//! `src/instructions/x86.instr`), and the peephole's matchers from
//! `peephole.peep`.

// The tests read what build.rs does not.
#[allow(dead_code)]
#[path = "src/backend/isel/generator/mod.rs"]
mod generator;

fn main() {
    let rules = "src/backend/peephole.peep";
    let code16_forms = "../../target/llrm-x86-code16/src/instructions/x86.instr";
    for path in [rules, code16_forms, "src/backend/isel/generator", "../../target"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let read = |path: &std::path::Path| std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let mut targets: Vec<_> = std::fs::read_dir("../../target")
        .expect("crates/target")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|dir| dir.join("src/isel/patterns.isel").is_file())
        .collect();
    targets.sort();
    let mut index = String::new();
    let mut all = Vec::new();
    for dir in &targets {
        let directory = dir.file_name().and_then(|one| one.to_str()).expect("a name");
        let name = directory.strip_prefix("llrm-").unwrap_or(directory);
        let ident = name.replace('-', "_");
        let (patterns, forms) = (dir.join("src/isel/patterns.isel"), dir.join("src/instructions/x86.instr"));
        for path in [&patterns, &forms] {
            println!("cargo:rerun-if-changed={}", path.display());
        }
        let generated = generator::generate(&read(&forms), &read(&patterns), name).unwrap_or_else(|error| panic!("{name}: {error}"));
        std::fs::write(out.join(format!("isel_{ident}.rs")), generated.code).unwrap();
        index.push_str(&format!("pub mod {ident} {{\n    use super::*;\n    include!(concat!(env!(\"OUT_DIR\"), \"/isel_{ident}.rs\"));\n}}\n\n"));
        all.push(format!("&{ident}::SELECTOR"));
    }
    index.push_str(&format!("/// Every target's selector, by its directory's name.\npub static ALL: [&Compiled; {}] = [{}];\n", all.len(), all.join(", ")));
    std::fs::write(out.join("selectors.rs"), index).unwrap();

    let made = match llrm_peepgen::generate(&read(std::path::Path::new(code16_forms)), "x86.instr", &read(std::path::Path::new(rules)), "peephole.peep") {
        Ok(made) => made,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    std::fs::write(out.join("peephole.rs"), made.rules).unwrap();
}
