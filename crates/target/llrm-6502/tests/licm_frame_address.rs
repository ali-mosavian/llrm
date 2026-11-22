//! A frame address a loop makes on every trip is made once before it
//! (`MachineLICM` of a stack slot's `LEA`). Needs `llrm-cc` built (`LLRM_CC`).

use std::process::Command;

mod common;
use common::pipeline::llrm_cc;

fn assembly(level: &str) -> String {
    assembly_of("frame_address_loop", level)
}

fn assembly_of(name: &str, level: &str) -> String {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("licm-frame-address");
    std::fs::create_dir_all(&dir).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/data/{name}.c"));
    let out = dir.join(format!("{name}.{level}.s"));
    let made = Command::new(llrm_cc())
        .current_dir(&dir)
        .args(["--target=6502-sim65", level, "-fno-inline-functions", "-S", "-w", "-o"])
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// The lines of the loop: from its label to the branch (or, past a branch's
/// reach, the jump) back to it.
fn loop_body(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    for (at, line) in lines.iter().enumerate() {
        let Some(label) = line.trim().strip_prefix("bcc ").or_else(|| line.trim().strip_prefix("bne ")).or_else(|| line.trim().strip_prefix("jmp ")) else { continue };
        if let Some(start) = lines[..at].iter().position(|l| l.trim() == format!("{label}:")) {
            return lines[start..=at].to_vec();
        }
    }
    panic!("no loop in:\n{text}");
}

/// `b[i] = a[i] + 1` over two 70-byte locals: `a` is `rs0 + 70`, built with
/// `clc; lda rc0; adc #70; ...` on every trip (about 19 clocks a trip;
/// `stack_arrays_copy` x1.21 at -Os against llvm-mos).
#[test]
fn a_frame_address_is_made_before_the_loop_that_uses_it() {
    for level in ["-O2", "-Os"] {
        let text = assembly(level);
        let body = loop_body(&text);
        assert!(!body.iter().any(|l| l.trim() == "lda rc0"), "{level}: the loop reads rs0 every trip:\n{}", body.join("\n"));
    }
}

/// Nine values live across the loop leave no slot for the address: hoisting it
/// would raise the pressure past `MachineLICM`'s limit (`CanCauseHighRegPressure`),
/// so the `LEA` stays in the loop.
#[test]
fn a_frame_address_is_not_hoisted_into_a_loop_at_the_pressure_limit() {
    for level in ["-O2", "-Os"] {
        let text = assembly_of("frame_address_pressure", level);
        let body = loop_body(&text);
        assert!(body.iter().any(|l| l.trim() == "adc #70"), "{level}: the address left a loop with no register to spare:\n{}", body.join("\n"));
    }
}
