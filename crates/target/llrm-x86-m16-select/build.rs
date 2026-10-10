//! Generates x86-m16's selector, peephole rules and effect rows from its
//! definition directory, written against `llrm_core`.

use std::path::Path;

use llrm_iselgen::build;

fn main() {
    let groups_list = Path::new("../../backend/llrm-core/src/backend/peep/groups.list");
    println!("cargo:rerun-if-changed={}", groups_list.display());
    let groups = build::rule_groups(&std::fs::read_to_string(groups_list).expect("peep/groups.list"));
    let family = build::family(Path::new("../llrm-x86"));
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let built = build::target(Path::new("../llrm-x86-m16"), &family, &groups, "llrm_core", &out);
    std::fs::write(out.join("select.rs"), built.select_source()).unwrap();
}
