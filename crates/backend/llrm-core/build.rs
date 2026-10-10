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
    let mut peep = String::new();
    let mut all = Vec::new();
    // Without the fixtures feature nothing is generated for a target: the
    // select crates do that.
    for dir in targets.iter().filter(|_| fixtures) {
        let built = build::target(dir, &family, &groups, "crate", &out);
        let ident = &built.ident;
        index.push_str(&format!(
            "pub mod {ident} {{\n    use crate::backend::targets::{ident}::RULES;\n\n    include!(concat!(env!(\"OUT_DIR\"), \"/isel_{ident}.rs\"));\n}}\n\n"
        ));
        all.push(format!("&{ident}::SELECTOR"));
        peep.push_str(&built.rules);
    }
    std::fs::write(out.join("peep_targets.rs"), peep).unwrap();
    index.push_str(&format!(
        "/// Every target's selector, by its directory's name.\npub static ALL: [&Compiled; {}] = [{}];\n",
        all.len(),
        all.join(", ")
    ));
    std::fs::write(out.join("selectors.rs"), index).unwrap();
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
