//! `tools/fmt.sh --check` on a tree of its own: a comment past column 80 that
//! nothing can break fails it.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

/// fmt.sh's exit status and stdout on a one-package tree whose only source is
/// `source`.
fn check(
    name: &str,
    source: &str,
) -> (Option<i32>, String) {
    let root = std::env::temp_dir().join(format!("rfmt-post-fmt-sh-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let write = |path: &str, text: &str| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    let package = "[package]\nname = \"p\"\nversion = \"0.0.0\"\nedition = \"2024\"\n";
    write("Cargo.toml", "[workspace]\nmembers = [\"probe\"]\nresolver = \"3\"\n");
    write("probe/Cargo.toml", package);
    write("probe/src/lib.rs", source);
    write("tools/rfmt-post/Cargo.toml", &format!("{package}\n[workspace]\n"));
    write("tools/rfmt-post/src/lib.rs", "pub fn stub() {}\n");
    let repo = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    write("rustfmt.toml", &fs::read_to_string(format!("{repo}/rustfmt.toml")).unwrap());
    write("tools/fmt.sh", &fs::read_to_string(format!("{repo}/tools/fmt.sh")).unwrap());
    let script = root.join("tools/fmt.sh");
    // the workers run it by its name
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let out = Command::new(&script)
        .arg("--check")
        .env("RFMT_POST_BIN", env!("CARGO_BIN_EXE_rfmt-post"))
        .current_dir(&root)
        .output()
        .unwrap();
    fs::remove_dir_all(&root).unwrap();
    (out.status.code(), String::from_utf8(out.stdout).unwrap())
}

const LONG: &str = "a comment far longer than eighty columns, which no line break can bring under it unless it moves";

/// `--check` said "clean" for a comment of 100 columns it had no way to break,
/// so the rule would have decayed.
#[test]
fn check_fails_on_a_comment_past_column_80_that_nothing_can_break() {
    let (code, out) = check("stays", &format!("pub fn f() -> &'static str {{\n    \"a\nb\" // {LONG}\n}}\n"));
    assert_eq!(code, Some(1));
    assert_eq!(out, "probe/src/lib.rs:3: comment past column 80\n");
}

#[test]
fn check_passes_a_comment_that_fits_after_the_formatter_broke_it() {
    let (code, out) = check("broken", &format!("pub fn f() {{\n    // {LONG}\n}}\n"));
    assert_eq!(code, Some(1), "the unbroken comment is a file to format");
    assert_eq!(out, "probe/src/lib.rs\n");
}

#[test]
fn check_passes_a_word_wider_than_the_limit() {
    let url = "https://github.com/ali-mosavian/llrm/issues/794/with/a/path/that/runs/on/and/on/past/column/eighty";
    let (code, out) = check("url", &format!("//! {url}\n\npub fn f() {{}}\n"));
    assert_eq!((code, out.as_str()), (Some(0), ""));
}
