//! `.eh_frame` describes the stack pointer as the code moves it.

#[path = "common/target_dir.rs"]
mod target_dir;

use std::path::Path;
use std::process::Command;

fn tool(
    program: &str,
    args: &[&str],
    file: &Path,
) -> Option<String> {
    let out = Command::new(program).args(args).arg(file).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The bytes `function` moves the stack pointer down by: pushes and
/// `sub $n,%rsp`, up to its first `ret`.
fn moved(
    disassembly: &str,
    function: &str,
) -> u64 {
    let mut total = 0;
    let mut inside = false;
    for line in disassembly.lines() {
        if line.contains(&format!("<{function}>:")) {
            inside = true;
        } else if inside && line.ends_with(">:") {
            break;
        } else if inside {
            let text = line.split('\t').nth(2).unwrap_or("").trim();
            if text.starts_with("ret") {
                break;
            }
            if text.starts_with("push") {
                total += 8;
            } else if let Some(rest) = text.strip_prefix("sub") {
                if let Some(n) = rest.trim().strip_prefix('$').and_then(|r| r.split(',').next()) {
                    if rest.contains("%rsp") {
                        total += u64::from_str_radix(n.trim_start_matches("0x"), 16).unwrap_or(0);
                    }
                }
            }
        }
    }
    total
}

/// The deepest CFA offset the FDE covering `function` describes, less the
/// return address.
fn described(
    frames: &str,
    function: &str,
    start: u64,
) -> u64 {
    let _ = function;
    let mut inside = false;
    let mut deepest = 0;
    for line in frames.lines() {
        if line.contains("FDE") {
            inside = line.contains(&format!("pc={start:08x}.."));
        } else if inside {
            if let Some(n) = line.split("DW_CFA_def_cfa_offset:").nth(1) {
                deepest = deepest.max(n.trim().parse::<u64>().unwrap_or(0));
            }
        }
    }
    deepest.saturating_sub(8)
}

/// A frame the unwinder is told about must be the one the code builds: a leaf
/// that reserves its locals below the stack pointer must not describe a CFA
/// move that is not there, and one that does move it must describe all of it.
#[test]
fn eh_frame_describes_the_stack_pointer_moves_the_code_makes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let object = std::env::temp_dir().join(format!("eh_frame_{}.o", std::process::id()));
    let made = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", target_dir::of(&root).join("release/llrm-llc"))
        .args(["-c", "-m64", "-O2", "-fomit-frame-pointer", "-fobject-format=elf"])
        .arg(root.join("tests/cc/eh_frame/leaf.c"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let (Some(code), Some(frames), Some(symbols)) = (
        tool("objdump", &["-d", "--no-show-raw-insn", "-w"], &object),
        tool("readelf", &["--debug-dump=frames"], &object),
        tool("nm", &[], &object),
    ) else {
        return;
    };
    let _ = std::fs::remove_file(&object);
    for function in ["leaf", "caller"] {
        let start = symbols
            .lines()
            .find(|line| line.ends_with(&format!(" {function}")))
            .and_then(|line| u64::from_str_radix(line.split(' ').next()?, 16).ok())
            .expect("the function's address");
        let (made, said) = (moved(&code, function), described(&frames, function, start));
        assert_eq!(said, made, "{function}: the code moves rsp by {made}, .eh_frame says {said}\n{frames}");
    }
}
