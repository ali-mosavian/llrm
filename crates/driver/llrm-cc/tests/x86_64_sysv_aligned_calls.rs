//! `x86_64-linux`: every call finds rsp a multiple of 16 (System V), also a
//! call with one argument on the stack. `call_depths` reads the depth at each
//! call from the listing;
//! `an_m64_call_passes_the_sysv_registers_and_one_stack_slot` once held the
//! area to 16 bytes, which `perf(m32): the aligned frame takes the call phase
//! that pads least` made the frame's job, leaving that test failing
//! on a tip that was correct. Needs `LLRM_LLC`.

use std::path::Path;
use std::process::Command;

/// The stack depth at each `call` of the straight-line function `name`, from
/// the entry (the return address is 8).
fn call_depths(
    listing: &str,
    name: &str,
) -> Vec<i64> {
    let start = listing.lines().position(|line| line.trim() == format!("{name} proc near")).expect("the function");
    let (mut depth, mut depths) = (8_i64, Vec::new());
    for line in listing.lines().skip(start + 1).map(str::trim).take_while(|line| *line != format!("{name} endp")) {
        let (op, rest) = line.split_once(' ').unwrap_or((line, ""));
        match op {
            "push" => depth += 8,
            "pop" => depth -= 8,
            "sub" | "add" if rest.starts_with("rsp, ") => {
                let n: i64 = rest["rsp, ".len()..].parse().expect("a constant frame step");
                depth += if op == "sub" { n } else { -n };
            }
            "call" => depths.push(depth),
            _ => {}
        }
    }
    depths
}

fn listing(level: &str) -> String {
    let llc = std::env::var_os("LLRM_LLC").expect("LLRM_LLC is not set: this test needs it and does not skip");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("sysv64-aligned-{level}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asm = dir.join("a.s");
    let run = Command::new(env!("CARGO_BIN_EXE_llrm-cc-bin"))
        .env("LLRM_CC_TARGET", "x86_64-linux")
        .env("LLRM_LLC", llc)
        .args([level, "-w", "-S", "-o"])
        .arg(&asm)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/c/x86_64_stack_args_calls.c"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    std::fs::read_to_string(&asm).unwrap()
}

#[test]
fn every_call_with_a_stack_argument_finds_rsp_a_multiple_of_16() {
    for level in ["-O1", "-O2", "-Os"] {
        let text = listing(level);
        for (name, calls) in [("one_call", 1), ("two_calls_and_locals", 2), ("odd_locals", 1)] {
            let depths = call_depths(&text, name);
            assert_eq!(depths.len(), calls, "{level} {name}:\n{text}");
            // the seven-argument call's one stack slot is above the depth
            assert!(depths.iter().all(|depth| depth % 16 == 0), "{level} {name}: {depths:?}\n{text}");
        }
    }
}

/// The reader must see a misaligned call as one: an entry with no frame is
/// 8 mod 16.
#[test]
fn the_depth_reader_sees_a_misaligned_call() {
    let text = "f proc near\n    call g\n    ret\nf endp\ng2 proc near\n    push rbx\n    call g\n    pop rbx\n    ret\ng2 endp\n";
    assert_eq!(call_depths(text, "f"), [8]);
    assert_eq!(call_depths(text, "g2"), [16]);
}
