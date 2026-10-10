//! What a build script does for a target crate: read its definition
//! directory and the family's, and write the target's instruction selector,
//! peephole rules as Rust (and `effect_rows`, the effect rows a data crate
//! carries). `core` is the path of the crate whose `backend::isel::api` and
//! `backend::peep` the generated code is written against: `crate` inside
//! `llrm-core`, `llrm_core` outside it.

use std::path::{Path, PathBuf};

use crate::generate;

/// The family's files every target's own are read after.
pub struct Family {
    pub forms: String,
    pub patterns: String,
    pub peephole: String,
}

fn read(path: &Path) -> String {
    println!("cargo:rerun-if-changed={}", path.display());
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The family's forms, selection patterns and peephole rules, under `dir`
/// (`llrm-x86`).
pub fn family(dir: &Path) -> Family {
    Family {
        forms: read(&dir.join("src/instructions/x86.instr")),
        patterns: read(&dir.join("src/isel/family.isel")),
        peephole: read(&dir.join("src/isel/peephole.peep")),
    }
}

/// What `target` wrote and returns for the including crate.
pub struct Built {
    /// `x86-m16`, as the definition directory is named without `llrm-`.
    pub name: String,
    /// `x86_m16`.
    pub ident: String,
    /// The code width the target's `datalayout.toml` names.
    pub mode: String,
    /// The module `<ident>` of the target's rules: the generated functions
    /// and the `RULES` that name them.
    pub rules: String,
}

/// Generates the target of `dir` (a directory `crates/target/llrm-x86-m16`)
/// into `out`: `isel_<ident>.rs`, `peephole_<ident>.rs`.
pub fn target(
    dir: &Path,
    family: &Family,
    groups: &[(String, bool)],
    core: &str,
    out: &Path,
) -> Built {
    let directory = dir.file_name().and_then(|one| one.to_str()).expect("a name");
    let name = directory.strip_prefix("llrm-").unwrap_or(directory).to_owned();
    let ident = name.replace('-', "_");
    let (patterns, forms) = (dir.join("src/isel/patterns.isel"), dir.join("src/instructions/x86.instr"));
    let forms = format!("{}\n{}", family.forms, read(&forms));
    let patterns = spliced(&family.patterns, &read(&patterns));
    let generated = generate(&forms, &patterns, &name, core).unwrap_or_else(|error| panic!("{name}: {error}"));
    std::fs::write(out.join(format!("isel_{ident}.rs")), generated.code).unwrap();
    let layout = read(&dir.join("src/machines/datalayout.toml"));
    let mode = layout
        .lines()
        .find_map(|line| line.strip_prefix("mode = "))
        .unwrap_or_else(|| panic!("{name}: datalayout.toml has no `mode`"))
        .trim()
        .to_owned();
    let reader = |path: &Path| read(path);
    let rules = peephole(dir, &ident, &forms, out, &reader, groups, &family.peephole, core);
    Built { name, ident, mode, rules }
}

impl Built {
    /// The source a target's select crate includes: its peephole `RULES`, its
    /// `SELECTOR` (which holds them) and the code width `MODE`, the same names
    /// for every target.
    pub fn select_source(&self) -> String {
        let ident = &self.ident;
        format!(
            "{rules}
pub use {ident}::RULES;

mod selection {{
    use super::RULES;

    include!(concat!(env!(\"OUT_DIR\"), \"/isel_{ident}.rs\"));
}}
pub use selection::SELECTOR;

/// The code width of the target, in bits.
pub const MODE: u32 = {mode};
",
            rules = self.rules,
            mode = self.mode
        )
    }
}

/// The target definition directories under `crates/target` (`dir`), sorted.
pub fn targets(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<_> = std::fs::read_dir(dir)
        .expect("crates/target")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|one| one.join("src/isel/patterns.isel").is_file())
        .collect();
    found.sort();
    found
}

/// The forms of the target in `dir`: the family's, then its own `x86.instr`.
pub fn forms(
    dir: &Path,
    family: &Family,
) -> String {
    format!("{}\n{}", family.forms, read(&dir.join("src/instructions/x86.instr")))
}

/// `row(name, dests, sources)` for a target's `x86.instr`: what each mnemonic's
/// forms read and write beyond their operands, and the flags (the table's, from
/// iced's Code of the mnemonic's forms).
pub fn effect_rows(forms: &str) -> String {
    let rows = crate::description::parse(forms).unwrap_or_else(|error| panic!("{error}"));
    let mut by_key: Vec<((String, usize, usize), Vec<String>)> = Vec::new();
    for form in &rows {
        let (flags_read, flags_written) = (form.flags_read, form.flags_written);
        let list = |names: &[String]| names.iter().map(|name| format!("{name:?}")).collect::<Vec<_>>().join(", ");
        let kinds = form
            .dests
            .iter()
            .chain(&form.sources)
            .map(|one| format!("{:?}", if one.tied.is_some() { "r" } else { one.kinds.as_str() }))
            .collect::<Vec<_>>()
            .join(", ");
        let ties = form
            .sources
            .iter()
            .enumerate()
            .filter_map(|(at, one)| one.tied.map(|dest| format!("({at}, {dest})")))
            .collect::<Vec<_>>()
            .join(", ");
        let pins = form
            .fixed
            .iter()
            .map(|(side, at, root)| format!("({}, {at}, {root:?})", *side == crate::description::Side::Dest))
            .collect::<Vec<_>>()
            .join(", ");
        let value = format!(
            "Row {{ reads: &[{}], writes: &[{}], kinds: &[{kinds}], width: {}, ties: &[{ties}], pins: &[{pins}], flags_read: {flags_read:#x}, flags_written: {flags_written:#x} }}",
            list(&form.reads),
            list(&form.writes),
            form.widths.first().copied().unwrap_or(0)
        );
        let key = (form.name.clone(), form.dests.len(), form.sources.len());
        match by_key.iter_mut().find(|(one, _)| *one == key) {
            Some((_, values)) => {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
            None => by_key.push((key, vec![value])),
        }
    }
    let mut code = String::from("// @generated by build.rs from x86.instr.\nuse llrm_lir::registers::Row;\n\n");
    code.push_str("pub fn rows(name: &str, dests: usize, sources: usize) -> &'static [Row] {\n    match (name, dests, sources) {\n");
    for ((name, dests, sources), values) in &by_key {
        code.push_str(&format!("        ({name:?}, {dests}, {sources}) => &[{}],\n", values.join(", ")));
    }
    code.push_str("        _ => &[],\n    }\n}\n");
    code
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
    core: &str,
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
        &format!("&{core}::backend::peep::NO_NAMES")
    };
    format!(
        "pub mod {ident} {{
    #[allow(clippy::all, unused_imports, unused_variables, unreachable_patterns, dead_code)]
    mod generated {{
        use std::sync::Arc;

        use iced_x86::{{Register, RflagsBits}};

        use {core}::backend::lanes::Lanes;
        use {core}::backend::peep::walk::{{self, Cx, Facts, Kind, Matcher, Out, Rewrite, Side, Skip, Window}};
        use {core}::backend::peep::{{Set, field, guards, make}};
        use {core}::model::ir::{{Imm, Loc, Operation, Semantics}};
        use {core}::model::lir::{{self, Insn, LirBody}};

        include!(concat!(env!(\"OUT_DIR\"), \"/peephole_{ident}.rs\"));
    }}

    use {core}::backend::peep::Rules;

    pub static RULES: Rules = Rules {{
{rules_text}        zero_jcc: {zero_jcc},
    }};
}}

"
    )
}

/// The groups of `peep/groups.list`: each name and whether the schedule runs
/// its `insns` form (else its body form).
pub fn rule_groups(text: &str) -> Vec<(String, bool)> {
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

/// `family` with each `own NAME` line replaced by the text under `splice NAME`
/// in `own`; a splice the family has no line for is an error, not a dropped
/// pattern.
pub fn spliced(
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
