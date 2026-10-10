//! Generates one instruction selector per target from its definition
//! directory (`crates/target/<name>/src/isel/patterns.isel` against
//! `src/instructions/x86.instr`) and its peephole rules: the family's
//! (`llrm-x86`) and its own `src/isel/peephole.peep`.

use llrm_iselgen::build;

fn main() {
    // Files, not `crates/target`: cargo scans a watched directory for the
    // newest mtime in it, and tools leave files there. A new target is a
    // new workspace member, so Cargo.lock says so.
    for path in ["../llrm-iselgen/src", "../../../Cargo.lock"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let family = build::family(std::path::Path::new("../../target/llrm-x86"));
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let fixtures = std::env::var_os("CARGO_FEATURE_FIXTURES").is_some();
    let targets = build::targets(std::path::Path::new("../../target"));
    println!("cargo:rerun-if-changed=src/backend/peep/groups.list");
    let groups =
        build::rule_groups(&std::fs::read_to_string("src/backend/peep/groups.list").expect("peep/groups.list"));
    std::fs::write(out.join("peep_rules.rs"), rules_type(&groups)).unwrap();
    let mut index = String::new();
    let mut effects = String::new();
    let mut modes: Vec<(String, String)> = Vec::new();
    let mut peep = String::new();
    let mut all = Vec::new();
    // Without the fixtures feature nothing is generated for a target: the
    // select crates do that.
    for dir in targets.iter().filter(|_| fixtures) {
        let built = build::target(dir, &family, &groups, "crate", &out);
        let ident = &built.ident;
        effects.push_str(&format!(
            "pub mod {ident} {{\n    include!(concat!(env!(\"OUT_DIR\"), \"/effects_{ident}.rs\"));\n}}\n\n"
        ));
        index.push_str(&format!(
            "pub mod {ident} {{\n    use crate::backend::targets::{ident}::RULES;\n\n    include!(concat!(env!(\"OUT_DIR\"), \"/isel_{ident}.rs\"));\n}}\n\n"
        ));
        all.push(format!("&{ident}::SELECTOR"));
        peep.push_str(&built.rules);
        modes.push((built.ident.clone(), built.mode.clone()));
    }
    std::fs::write(out.join("peep_targets.rs"), peep).unwrap();
    std::fs::write(out.join("positional.rs"), positional(&targets)).unwrap();
    std::fs::write(out.join("register_info.rs"), register_info(&targets)).unwrap();
    index.push_str(&format!(
        "/// Every target's selector, by its directory's name.\npub static ALL: [&Compiled; {}] = [{}];\n",
        all.len(),
        all.join(", ")
    ));
    std::fs::write(out.join("selectors.rs"), index).unwrap();
    // The table of the target whose code is `bits` bits, by what its
    // description says.
    effects.push_str("pub fn rows_for(bits: u32) -> Option<crate::backend::effects::Rows> {\n");
    for (ident, mode) in &modes {
        effects.push_str(&format!("    if bits == {mode} {{\n        return Some({ident}::rows);\n    }}\n"));
    }
    effects.push_str("    None\n}\n");
    std::fs::write(out.join("effects.rs"), effects).unwrap();
}

/// The register file every target's `registers.regs` states, as the table
/// `backend::registerinfo` queries: one entry per register, by iced's number.
/// The targets state the same registers (checked here: name, width, root and
/// lane). The classes are those every target listing the register gives it
/// (`base` and `index` differ by target and are not here); the debug-format
/// numbers are each target's own.
fn register_info(targets: &[std::path::PathBuf]) -> String {
    struct Row {
        name: String,
        bits: u32,
        root: String,
        lane: u32,
        classes: Vec<String>,
    }
    let mut rows: Vec<Row> = Vec::new();
    for dir in targets {
        let path = dir.join("src/registers.regs");
        println!("cargo:rerun-if-changed={}", path.display());
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let columns: Vec<&str> = line.split_whitespace().collect();
            if columns.len() != 7 {
                continue;
            }
            let classes: Vec<String> =
                if columns[4] == "-" { Vec::new() } else { columns[4].split(',').map(str::to_owned).collect() };
            let (bits, lane) = (columns[1].parse().unwrap(), columns[3].parse().unwrap());
            match rows.iter_mut().find(|one| one.name == columns[0]) {
                Some(seen) => {
                    assert_eq!(
                        (seen.bits, &seen.root, seen.lane),
                        (bits, &columns[2].to_owned(), lane),
                        "{}: {} differs between targets",
                        path.display(),
                        columns[0]
                    );
                    // Only the classes every target gives it.
                    seen.classes.retain(|one| classes.contains(one));
                }
                None => {
                    rows.push(Row { name: columns[0].to_owned(), bits, root: columns[2].to_owned(), lane, classes })
                }
            }
        }
    }
    let mut code = String::from(
        "pub static TABLE: [Option<Entry>; 256] = {\n    let mut table: [Option<Entry>; 256] = [None; 256];\n",
    );
    for row in &rows {
        let classes = row.classes.iter().map(|one| format!("{one:?}")).collect::<Vec<_>>().join(", ");
        code.push_str(&format!(
            "    table[iced_x86::Register::{} as usize] = Some(Entry {{ name: {:?}, bits: {}, root: iced_x86::Register::{}, lane: {}, classes: &[{classes}] }});\n",
            row.name.to_uppercase(),
            row.name,
            row.bits,
            row.root.to_uppercase(),
            row.lane
        ));
    }
    code.push_str("    table\n};\n");
    code
}

/// `positional(register)` for the registers whose class is `positional` in
/// every target's `registers.regs`: a position in a stack, which no pass may
/// rename or drop. The targets state the same set (checked here); iced names
/// them.
fn positional(targets: &[std::path::PathBuf]) -> String {
    let mut sets: Vec<(String, Vec<String>)> = Vec::new();
    for dir in targets {
        let path = dir.join("src/registers.regs");
        println!("cargo:rerun-if-changed={}", path.display());
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let names: Vec<String> = text
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
            .filter_map(|line| {
                let columns: Vec<&str> = line.split_whitespace().collect();
                columns
                    .get(4)
                    .filter(|classes| classes.split(',').any(|one| one == "positional"))
                    .map(|_| columns[0].to_owned())
            })
            .collect();
        sets.push((path.display().to_string(), names));
    }
    let (first, rest) = sets.split_first().expect("a target");
    for other in rest {
        assert_eq!(other.1, first.1, "{} and {} state different positional registers", other.0, first.0);
    }
    let names: Vec<String> =
        first.1.iter().map(|name| format!("iced_x86::Register::{}", name.to_uppercase())).collect();
    let test = if names.is_empty() { "false".to_owned() } else { format!("matches!(register, {})", names.join(" | ")) };
    format!(
        "/// Generated by build.rs from the targets' registers.regs.\npub fn positional(register: iced_x86::Register) -> bool {{\n    let _ = register;\n    {test}\n}}\n"
    )
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
