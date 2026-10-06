//! Generates one instruction selector per target from its definition
//! directory (`crates/target/<name>/src/isel/patterns.isel` against
//! `src/instructions/x86.instr`) and its peephole rules from `src/isel/peephole.peep`.

// The tests read what build.rs does not.
#[allow(dead_code)]
#[path = "src/backend/isel/generator/mod.rs"]
mod generator;

fn main() {
    for path in ["src/backend/isel/generator", "../../target"] {
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
    let mut peep = String::new();
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
        peep.push_str(&peephole(dir, &ident, &forms, &out, &read));
    }
    std::fs::write(out.join("peep_targets.rs"), peep).unwrap();
    index.push_str(&format!("/// Every target's selector, by its directory's name.\npub static ALL: [&Compiled; {}] = [{}];\n", all.len(), all.join(", ")));
    std::fs::write(out.join("selectors.rs"), index).unwrap();

}

/// The rule groups `peep::Rules` has a field for, and those of them that also
/// have a form over a window of instructions (`<group>_insns`).
const GROUPS: [&str; 17] = [
    "extensions", "pushed_constants", "pushes", "narrowed_moves", "commuted", "transferred", "shuttles", "restored_copies", "fused", "far_loads", "increments",
    "borrows", "doubled", "zero_compares", "memory_arguments", "paired_pushes", "immediate_arguments",
];

/// The module `peep::targets::<ident>`: the target's rules, generated from its
/// `peephole.peep` if it has one, and the `RULES` that name them.
fn peephole(dir: &std::path::Path, ident: &str, forms: &std::path::Path, out: &std::path::Path, read: &dyn Fn(&std::path::Path) -> String) -> String {
    let rules = dir.join("src/isel/peephole.peep");
    if !rules.is_file() {
        return format!("pub mod {ident} {{\n    use super::super::Rules;\n\n    pub static RULES: Rules = Rules::NONE;\n}}\n\n");
    }
    println!("cargo:rerun-if-changed={}", rules.display());
    let made = llrm_peepgen::generate(&read(forms), "x86.instr", &read(&rules), "peephole.peep").unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });
    for (group, _, _) in &made.groups {
        assert!(GROUPS.contains(&group.as_str()), "{}: group `{group}` has no field in peep::Rules", rules.display());
    }
    let field = |name: &str| if made.groups.iter().any(|(group, _, _)| group == name) { format!("Some(generated::{name})") } else { "None".to_owned() };
    let window = |name: &str| if made.rules.contains(&format!("pub fn {name}_insns(")) { format!("Some(generated::{name}_insns)") } else { "None".to_owned() };
    std::fs::write(out.join(format!("peephole_{ident}.rs")), &made.rules).unwrap();
    let mut rules_text = String::new();
    for name in GROUPS {
        rules_text.push_str(&format!("        {name}: {},\n        {name}_insns: {},\n", field(name), window(name)));
    }
    let zero_jcc = if made.rules.contains("pub static SET_ZERO_JCC") { "&generated::SET_ZERO_JCC" } else { "&super::super::NO_NAMES" };
    format!(
        "pub mod {ident} {{
    #[allow(clippy::all, unused_imports, unused_variables, unreachable_patterns, dead_code)]
    mod generated {{
        use std::sync::Arc;

        use iced_x86::{{Register, RflagsBits}};

        use crate::backend::lanes::Lanes;
        use crate::backend::peep::walk::{{self, Cx, Facts, Kind, Matcher, Out, Rewrite, Side, Skip, Window}};
        use crate::backend::peep::{{Set, field, guards, make}};
        use crate::model::ir::{{Imm, Loc, Operation, Semantics}};
        use crate::model::lir::{{self, Insn, LirBody}};

        include!(concat!(env!(\"OUT_DIR\"), \"/peephole_{ident}.rs\"));
    }}

    use super::super::Rules;

    pub static RULES: Rules = Rules {{
{rules_text}        zero_jcc: {zero_jcc},
    }};
}}

"
    )
}
