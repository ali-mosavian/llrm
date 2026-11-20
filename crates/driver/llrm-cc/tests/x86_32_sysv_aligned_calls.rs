//! `x86_32-linux` passes `-mstack-alignment=16`: every call finds esp a
//! multiple of 16, as i386 System V, gcc, clang and glibc's SSE code assume. A
//! function without a double kept 8: `h()` was called at esp = 8 mod 16,
//! and clang objects mixed with ours faulted on an aligned SSE access. Rows
//! without the flag (x86_32-dos) keep their frames. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

/// The stack depth at each `call`, from the entry (the return address is 4), of
/// the straight-line function `name` in an `llrm-cc -S` listing.
fn call_depths(
    listing: &str,
    name: &str,
) -> Vec<i64> {
    let start = listing.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    let (mut depth, mut depths) = (4_i64, Vec::new());
    for line in listing.lines().skip(start + 1).map(str::trim).take_while(|line| *line != format!("{name} endp")) {
        let (op, rest) = line.split_once(' ').unwrap_or((line, ""));
        match op {
            "push" | "pushd" => depth += 4,
            "pop" => depth -= 4,
            "sub" | "add" if rest.starts_with("esp, ") => {
                let n: i64 = rest["esp, ".len()..].parse().expect("a constant frame step");
                depth += if op == "sub" { n } else { -n };
            }
            "call" => depths.push(depth),
            _ => {}
        }
    }
    depths
}

fn depths_of(
    target: &str,
    level: &str,
    function: &str,
) -> (Vec<i64>, String) {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("sysv-aligned-{target}-{function}-{level}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", target)
        .env("LLRM_LLC", &llc)
        .args(["-march=i486", level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/sysv_aligned_calls.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let listing = std::fs::read_to_string(&asm).unwrap();
    let name = if target == "x86_32-dos" { format!("_{function}") } else { function.to_owned() };
    (call_depths(&listing, &name), listing)
}

fn depths(
    target: &str,
    level: &str,
) -> (Vec<i64>, String) {
    depths_of(target, level, "f")
}

#[test]
fn every_call_of_a_function_without_doubles_finds_the_stack_16_aligned() {
    for level in ["-O1", "-O2", "-Os"] {
        let (depths, listing) = depths("x86_32-linux", level);
        assert_eq!(depths.len(), 2, "{level}:\n{listing}");
        assert!(depths.iter().all(|depth| depth % 16 == 0), "{level}: depths {depths:?}\n{listing}");
    }
}

#[test]
fn a_row_without_the_flag_keeps_its_frames() {
    let (depths, listing) = depths("x86_32-dos", "-Os");
    assert_eq!(depths.len(), 2, "{listing}");
    assert!(depths.iter().any(|depth| depth % 16 != 0), "padded without the flag: {depths:?}\n{listing}");
}

/// Each of two 8-byte calls was padded (`sub esp, 8` before and `add esp, 16`
/// after) and the frame was padded again in the prologue: QuickrBasic m32 -Os
/// +4.75% against no alignment. The frame leaves the stack where the calls' own
/// bytes align it, and nothing is padded.
#[test]
fn calls_that_push_alike_pad_neither_the_frame_nor_the_calls() {
    for level in ["-O1", "-O2", "-Os"] {
        let (depths, listing) = depths_of("x86_32-linux", level, "pair");
        assert_eq!(depths.len(), 2, "{level}:\n{listing}");
        assert!(depths.iter().all(|depth| depth % 16 == 0), "{level}: depths {depths:?}\n{listing}");
        let start = listing.lines().position(|line| line.trim() == "pair proc near").unwrap();
        let body: Vec<&str> = listing.lines().skip(start).take_while(|line| line.trim() != "pair endp").collect();
        assert!(!body.iter().any(|line| line.trim().starts_with("sub esp")), "{level}:\n{}", body.join("\n"));
    }
}
