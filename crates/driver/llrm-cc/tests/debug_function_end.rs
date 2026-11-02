//! A function's debug range ends where its code ends.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn run(program: &str, args: &[&str], file: &Path) -> Option<String> {
    let out = Command::new(program).args(args).arg(file).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `(name, DW_AT_high_pc)` of each subprogram, as `readelf` prints the info section.
fn high_pcs(info: &str) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let (mut name, mut high, mut inside) = (String::new(), None, false);
    let mut flush = |name: &mut String, high: &mut Option<u64>, out: &mut Vec<(String, u64)>| {
        if let Some(high) = high.take() {
            out.push((std::mem::take(name), high));
        }
    };
    for line in info.lines() {
        if line.contains("Abbrev Number") {
            flush(&mut name, &mut high, &mut out);
            inside = line.contains("DW_TAG_subprogram");
        } else if inside && line.contains("DW_AT_name") {
            name = line.rsplit(": ").next().unwrap_or("").trim().to_owned();
        } else if inside && line.contains("DW_AT_high_pc") {
            high = line.rsplit(": ").next().and_then(|n| u64::from_str_radix(n.trim().trim_start_matches("0x"), 16).ok());
        }
    }
    flush(&mut name, &mut high, &mut out);
    out
}

/// The object file holds each function followed by alignment padding. The debug
/// range of the function ran to the next function's start, so it covered the
/// padding: `DW_AT_high_pc` was larger than the function (`.size`) by the nops,
/// and a debugger listed them as part of the function.
#[test]
fn a_functions_debug_range_ends_at_its_size_not_at_the_next_function() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let object = std::env::temp_dir().join(format!("debug_end_{}.o", std::process::id()));
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-cc"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-c", "-m64", "-O2", "-g", "-fobject-format=elf"])
        .arg(root.join("tests/cc/eh_frame/leaf.c"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let (Some(info), Some(symbols)) =
        (run("readelf", &["--debug-dump=info"], &object), run("readelf", &["-sW"], &object))
    else {
        return;
    };
    let _ = std::fs::remove_file(&object);
    let size = |name: &str| -> u64 {
        let row = symbols.lines().find(|line| line.split_whitespace().last() == Some(name)).expect("the symbol");
        row.split_whitespace().nth(2).unwrap().parse().unwrap()
    };
    let found = high_pcs(&info);
    assert!(found.len() >= 2, "{found:?}");
    for (name, high) in found {
        assert_eq!(high, size(&name), "{name}: debug range {high}, code {}", size(&name));
    }
}
