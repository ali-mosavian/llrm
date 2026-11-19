//! Embeds toolchain/clang (the headers and header directories targets.toml's
//! recipes include) so a dist still compiles after the checkout it was built in
//! is moved or deleted.
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn walk(
    dir: &Path,
    base: &Path,
    out: &mut Vec<(String, PathBuf)>,
) {
    let mut entries: Vec<_> =
        std::fs::read_dir(dir).expect("toolchain/clang is readable").map(|e| e.expect("entry").path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(&path, base, out);
        } else {
            let name = path.strip_prefix(base).expect("under base").to_string_lossy().replace('\\', "/");
            out.push((name, path));
        }
    }
}

fn main() {
    let root = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it")).join("../../../toolchain/clang");
    let root = root.canonicalize().expect("toolchain/clang exists");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    let mut text = String::from("pub const TOOLCHAIN_FILES: &[(&str, &[u8])] = &[\n");
    for (name, path) in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        writeln!(text, "    ({name:?}, include_bytes!({:?})),", path.display().to_string()).expect("string write");
    }
    text.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets it")).join("toolchain.rs");
    std::fs::write(out, text).expect("OUT_DIR is writable");
}
