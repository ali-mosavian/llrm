//! Generates one instruction selector per target from its definition
//! directory (`crates/target/<name>/src/isel/patterns.isel` against
//! `src/instructions/x86.instr`) and its peephole rules: the family's
//! (`llrm-x86`) and its own `src/isel/peephole.peep`.

// The tests read what build.rs does not.
#[allow(dead_code)]
#[path = "src/backend/isel/generator/mod.rs"]
mod generator;

fn main() {
    let family_forms = "../../target/llrm-x86/src/instructions/x86.instr";
    let family_patterns = "../../target/llrm-x86/src/isel/family.isel";
    let family_peephole = "../../target/llrm-x86/src/isel/peephole.peep";
    // Files, not `crates/target`: cargo scans a watched directory for the
    // newest mtime in it, and tools leave files there. A new target is a
    // new workspace member, so Cargo.lock says so.
    for path in ["src/backend/isel/generator", "../../../Cargo.lock", family_forms, family_patterns, family_peephole] {
        println!("cargo:rerun-if-changed={path}");
    }
    let read = |path: &std::path::Path| {
        std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    };
    // A target's forms are the family's, then its own.
    let forms_of = |own: &std::path::Path| format!("{}\n{}", read(std::path::Path::new(family_forms)), read(own));
    // And its patterns: the family's file with each `own NAME` line replaced by
    // the target's patterns under `splice NAME`, so where they go is the
    // family's to say.
    let patterns_of = |own: &std::path::Path| spliced(&read(std::path::Path::new(family_patterns)), &read(own));
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let mut targets: Vec<_> = std::fs::read_dir("../../target")
        .expect("crates/target")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|dir| dir.join("src/isel/patterns.isel").is_file())
        .collect();
    targets.sort();
    let groups = rule_groups(&read(std::path::Path::new("src/backend/peep/groups.list")));
    println!("cargo:rerun-if-changed=src/backend/peep/groups.list");
    std::fs::write(out.join("peep_rules.rs"), rules_type(&groups)).unwrap();
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
        let generated = generator::generate(&forms_of(&forms), &patterns_of(&patterns), name)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        std::fs::write(out.join(format!("isel_{ident}.rs")), generated.code).unwrap();
        index.push_str(&format!("pub mod {ident} {{\n    use super::*;\n    include!(concat!(env!(\"OUT_DIR\"), \"/isel_{ident}.rs\"));\n}}\n\n"));
        all.push(format!("&{ident}::SELECTOR"));
        peep.push_str(&peephole(
            dir,
            &ident,
            &forms_of(&forms),
            &out,
            &read,
            &groups,
            &read(std::path::Path::new(family_peephole)),
        ));
    }
    std::fs::write(out.join("peep_targets.rs"), peep).unwrap();
    index.push_str(&format!(
        "/// Every target's selector, by its directory's name.\npub static ALL: [&Compiled; {}] = [{}];\n",
        all.len(),
        all.join(", ")
    ));
    std::fs::write(out.join("selectors.rs"), index).unwrap();
}

/// The module `peep::targets::<ident>`: the target's rules, generated from its
/// `peephole.peep` if it has one, and the `RULES` that name them.
fn peephole(
    dir: &std::path::Path,
    ident: &str,
    forms: &str,
    out: &std::path::Path,
    read: &dyn Fn(&std::path::Path) -> String,
    groups: &[(String, bool)],
    family: &str,
) -> String {
    let rules = dir.join("src/isel/peephole.peep");
    // The family's rules, then the target's own where it has any.
    let own = if rules.is_file() {
        println!("cargo:rerun-if-changed={}", rules.display());
        read(&rules)
    } else {
        String::new()
    };
    let made = llrm_peepgen::generate(forms, "x86.instr", &format!("{family}\n{own}"), "peephole.peep").unwrap_or_else(
        |error| {
            eprintln!("{error}");
            std::process::exit(1);
        },
    );
    for (group, _, _) in &made.groups {
        assert!(
            groups.iter().any(|(name, _)| name == group),
            "{}: group `{group}` is not in peep/groups.list",
            rules.display()
        );
    }
    let mut rules_text = String::new();
    for (name, insns) in groups {
        let function = if *insns { format!("{name}_insns") } else { name.clone() };
        let made_here = made.groups.iter().any(|(group, _, _)| group == name);
        let value = match made_here {
            false => "None".to_owned(),
            true => {
                assert!(
                    made.rules.contains(&format!("pub fn {function}(")),
                    "{}: group `{name}` has no `{function}`, the form the schedule runs",
                    rules.display()
                );
                format!("Some(generated::{function})")
            }
        };
        rules_text.push_str(&format!("        {name}: {value},\n"));
    }
    std::fs::write(out.join(format!("peephole_{ident}.rs")), &made.rules).unwrap();
    let zero_jcc = if made.rules.contains("pub static SET_ZERO_JCC") {
        "&generated::SET_ZERO_JCC"
    } else {
        "&super::super::NO_NAMES"
    };
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

/// The groups of `peep/groups.list`: each name and whether the schedule runs
/// its `insns` form (else its body form).
fn rule_groups(text: &str) -> Vec<(String, bool)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| match line.split_once(' ') {
            Some((name, "body")) => (name.to_owned(), false),
            Some((name, "insns")) => (name.to_owned(), true),
            _ => panic!("peep/groups.list: `{line}` is not `<group> body|insns`"),
        })
        .collect()
}

/// `peep::Rules`, `Rules::NONE` and the group names, from the list.
fn rules_type(groups: &[(String, bool)]) -> String {
    let kind = |insns: bool| if insns { "InsnRule" } else { "BodyRule" };
    let fields: String =
        groups.iter().map(|(name, insns)| format!("    pub {name}: Option<{}>,\n", kind(*insns))).collect();
    let none: String = groups.iter().map(|(name, _)| format!("        {name}: None,\n")).collect();
    let present: String = groups
        .iter()
        .map(|(name, _)| format!("        if self.{name}.is_some() {{\n            out.push({name:?});\n        }}\n"))
        .collect();
    let names = groups.iter().map(|(name, _)| format!("{name:?}")).collect::<Vec<_>>().join(", ");
    format!(
        "/// What a target\'s `peephole.peep` made: each rule group the schedule runs, in the
/// form it runs, or none where the target has no such group. Bound to a target
/// by `llrm-driver` through its selector.
pub struct Rules {{
{fields}    /// The conditional jumps that test only a zero or a sign.
    pub zero_jcc: &'static Set,
}}

impl Rules {{
    /// A target with no peephole rules.
    pub const NONE: Rules = Rules {{
{none}        zero_jcc: &NO_NAMES,
    }};

    /// The groups the schedule runs.
    pub const GROUPS: [&'static str; {count}] = [{names}];

    /// The groups this target has.
    pub fn present(&self) -> Vec<&'static str> {{
        let mut out = Vec::new();
{present}        out
    }}
}}
",
        count = groups.len()
    )
}

/// `family` with each `own NAME` line replaced by the text under `splice NAME`
/// in `own`; a splice the family has no line for is an error, not a dropped
/// pattern.
fn spliced(
    family: &str,
    own: &str,
) -> String {
    let mut sections: Vec<(&str, String)> = Vec::new();
    for line in own.lines() {
        match line.strip_prefix("splice ") {
            Some(name) => sections.push((name.trim(), String::new())),
            None => {
                if let Some((_, text)) = sections.last_mut() {
                    text.push_str(line);
                    text.push('\n');
                }
            }
        }
    }
    let mut out = String::new();
    for line in family.lines() {
        match line.strip_prefix("own ") {
            Some(name) => {
                let at = sections.iter().position(|(one, _)| *one == name.trim());
                out.push_str(&at.map(|at| sections.remove(at).1).unwrap_or_default());
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    assert!(
        sections.is_empty(),
        "patterns spliced where the family has no `own` line: {:?}",
        sections.iter().map(|(name, _)| *name).collect::<Vec<_>>()
    );
    out
}
