//! A lane read out of a 64-bit vector (`<4 x i16>`, `<8 x i8>`, `<2 x i32>`,
//! `<2 x float>`) stopped selection (`CannotSelect`): only the 128-bit vectors
//! had a `vector_extract` pattern. The pin's `LowerEXTRACT_VECTOR_ELT` widens
//! a D register to a Q register first, then `umov`/`smov`/`mov` takes the lane.

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn compile(function: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{function}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{function}\n{}", String::from_utf8_lossy(&said.stderr));
    std::fs::read_to_string(&output).unwrap()
}

/// `%v`: the sum of two loads (a lane of a plain load is a scalar load).
fn sum_of_loads(
    vector: &str,
    op: &str,
) -> String {
    format!("  %a = load {vector}, ptr %p\n  %b = load {vector}, ptr %q\n  %v = {op} {vector} %a, %b\n")
}

/// An instruction `mnemonic dst, vN.<letter>[lane]` whose destination starts
/// with `dst` (`w`, `x`, `s`); register numbers are the allocator's.
fn reads_lane(
    text: &str,
    mnemonic: &str,
    dst: char,
    letter: char,
    lane: u32,
) -> bool {
    let tail = format!(".{letter}[{lane}]");
    text.lines().any(|line| {
        let mut parts = line.split_whitespace();
        parts.next() == Some(mnemonic)
            && parts.next().is_some_and(|d| d.starts_with(dst))
            && parts.next().is_some_and(|source| source.starts_with('v') && source.ends_with(&tail))
    })
}

/// `(vector type, element type, lanes)`.
const VECTORS: [(&str, &str, u32); 3] = [("<4 x i16>", "i16", 4), ("<8 x i8>", "i8", 8), ("<2 x i32>", "i32", 2)];

fn lane_letter(element: &str) -> char {
    match element {
        "i8" => 'b',
        "i16" => 'h',
        _ => 's',
    }
}

/// Every lane of every D-register integer vector, zero- and sign-extended to
/// `i32` and `i64`: one `umov`/`smov` of that lane, as the pin.
#[test]
fn every_lane_of_a_d_register_is_a_umov_or_smov() {
    for (vector, element, lanes) in VECTORS {
        let letter = lane_letter(element);
        for lane in 0..lanes {
            for (ext, to, reg, mov) in [
                ("zext", "i32", 'w', "umov"),
                ("sext", "i32", 'w', "smov"),
                ("zext", "i64", 'w', "umov"),
                ("sext", "i64", 'x', "smov"),
            ] {
                if element == "i32" && (ext == "zext" || to == "i32") {
                    continue;
                }
                let text = compile(&format!(
                    "define {to} @f(ptr %p, ptr %q) {{\n{}  %e = extractelement {vector} %v, i64 {lane}\n  \
                     %r = {ext} {element} %e to {to}\n  ret {to} %r\n}}\n",
                    sum_of_loads(vector, "add")
                ));
                assert!(
                    reads_lane(&text, mov, reg, letter, lane),
                    "{vector} lane {lane} {ext} to {to}: want a `{mov}` of the lane\n{text}"
                );
            }
        }
    }
}

/// A lane used as its own width (any-extended): `umov` for a byte or a half,
/// a word too, no spill.
#[test]
fn a_lane_used_as_an_element_is_a_move_out_of_the_vector() {
    for (vector, element, lanes) in VECTORS {
        let letter = lane_letter(element);
        for lane in 0..lanes {
            let text = compile(&format!(
                "define {element} @f(ptr %p, ptr %q) {{\n{}  %e = extractelement {vector} %v, i64 {lane}\n  \
                 %r = add {element} %e, 1\n  ret {element} %r\n}}\n",
                sum_of_loads(vector, "add")
            ));
            // `mov w, v.s[1]` is `umov` (the alias is the pin's text only).
            if element == "i32" && lane == 0 {
                assert!(text.contains("fmov"), "{vector} lane 0:\n{text}");
            } else {
                assert!(reads_lane(&text, "umov", 'w', letter, lane), "{vector} lane {lane}: want a `umov`\n{text}");
            }
            assert!(!text.contains("[sp"), "{vector} lane {lane} went through the stack:\n{text}");
        }
    }
}

/// A lane of a `<2 x float>` is the scalar register: lane 1 is `mov s, v.s[1]`.
#[test]
fn a_float_lane_of_a_d_register_is_a_dup() {
    let float = |lane: u32| {
        compile(&format!(
            "define float @f(ptr %p, ptr %q) {{\n{}  %e = extractelement <2 x float> %v, i64 {lane}\n  ret float %e\n}}\n",
            sum_of_loads("<2 x float>", "fadd")
        ))
    };
    let text = float(1);
    assert!(reads_lane(&text, "mov", 's', 's', 1), "{text}");
    let text = float(0);
    assert!(!text.contains("[sp") && !text.contains("mov"), "{text}");
}
