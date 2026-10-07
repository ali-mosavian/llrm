//! What the tests that assemble and link for the default target read of it: its `object.toml`, the one
//! place that says the object format, the listing's header and how a C program is linked.

use std::path::Path;

fn recipe() -> toml::Table {
    let target = llrm_driver::all().into_iter().find(|one| one.name() == llrm_driver::DEFAULT).expect("the default target is built in");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/target").join(format!("llrm-{}", target.name())).join("src/machines/object.toml");
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display())).parse().expect("object.toml parses")
}

/// The flag that makes jwasm write the object format the target writes (jwasm's own spelling of it).
pub fn assembler() -> &'static str {
    match recipe()["default"].as_str().expect("a default format") {
        "omf" => "-omf",
        other => panic!("jwasm has no flag for the {other} format"),
    }
}

/// The lines a listing opens with, as the target says: the memory model and the instruction set.
pub fn header() -> String {
    recipe()["header"].as_array().expect("a header").iter().map(|line| format!("{}\n", line.as_str().expect("a line"))).collect()
}

/// jwlink's arguments: the target's format words, then `rest`.
pub fn jwlink<'a>(rest: &[&'a str]) -> Vec<&'a str> {
    let format: Vec<&'static str> = recipe()["link"]["format"].as_array().expect("a link format").iter().map(|word| &*Box::leak(word.as_str().expect("a word").to_owned().into_boxed_str())).collect();
    format.into_iter().chain(rest.iter().copied()).collect()
}
