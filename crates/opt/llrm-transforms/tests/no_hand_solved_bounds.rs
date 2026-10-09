//! What the counted loops bound is the analysis manager's, kept and brought up to date: a pass that solved it by hand (`ranges::bounded`)
//! solved a function again for each pass that asked, six places and 5-26 Minstr of a -O1 compile (fpbench indvars, x_switch decide).

#[test]
fn no_pass_solves_what_the_loops_bound_by_hand() {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src")).expect("src reads").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(entry.path()).expect("a source reads");
        if text.lines().any(|line| !line.trim_start().starts_with("//") && line.contains("ranges::bounded(")) {
            found.push(name);
        }
    }
    assert!(found.is_empty(), "solved by hand: {found:?}; ask the manager's Bounded");
}
