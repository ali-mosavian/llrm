//! A build product that is committed by accident (a stray `x.obj` from a compile run in the repository root reached
//! main once) fails the gate here; the objects the tests read as input live under the fixture directories.

use std::process::Command;

const PRODUCTS: &[&str] = &["obj", "exe", "o", "map", "lst"];
const FIXTURES: &[&str] = &["tests/inputs/", "tests/fixtures/"];

#[test]
fn test_no_tracked_file_is_a_build_product_outside_the_fixtures() {
    let listed = Command::new("git").args(["ls-files", "-z"]).current_dir(env!("CARGO_MANIFEST_DIR")).output().expect("git runs");
    assert!(listed.status.success(), "git ls-files failed");
    let names = String::from_utf8(listed.stdout).expect("names are text");
    let strays: Vec<&str> = names
        .split('\0')
        .filter(|name| !name.is_empty())
        .filter(|name| name.rsplit_once('.').is_some_and(|(_, extension)| PRODUCTS.contains(&extension.to_ascii_lowercase().as_str())))
        .filter(|name| !FIXTURES.iter().any(|dir| name.starts_with(dir)))
        .collect();
    assert!(strays.is_empty(), "build products are tracked: {strays:?}");
}
