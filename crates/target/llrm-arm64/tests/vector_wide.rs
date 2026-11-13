//! Vectors wider than a register (`<16 x i64>`, `<8 x i64>`) in loops were
//! refused ("vector with no register class"): a phi carried one register, the
//! type legalizer had no `SplitVecRes_ExtendOp`, `CONCAT_VECTORS` of more than
//! two operands or `EXTRACT_SUBVECTOR`, and `v2i64` multiply had no lowering.

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn assembly(body: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{body}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

fn count(
    text: &str,
    mnemonic: &str,
    suffix: &str,
) -> usize {
    text.lines()
        .filter(|line| {
            line.trim().strip_prefix(mnemonic).is_some_and(|rest| rest.starts_with(' ') && rest.contains(suffix))
        })
        .count()
}

/// The lines of `mnemonic` (or its `2` form) whose destination has the
/// arrangement `suffix`.
fn count_dest(
    text: &str,
    mnemonic: &str,
    suffix: &str,
) -> usize {
    text.lines()
        .filter(|line| {
            let line = line.trim();
            let Some(rest) = line.strip_prefix(mnemonic) else { return false };
            let rest = rest.strip_prefix('2').unwrap_or(rest);
            rest.starts_with(' ') && rest.split(',').next().is_some_and(|dest| dest.ends_with(suffix))
        })
        .count()
}

/// The lines from the loop's label (the one a conditional branch goes back
/// to) to that branch.
fn loop_body(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let (at, label) = lines
        .iter()
        .enumerate()
        .find_map(|(at, line)| {
            let mnemonic = line.split_whitespace().next()?;
            if !(mnemonic.starts_with("b.") || matches!(mnemonic, "cbz" | "cbnz" | "tbz" | "tbnz")) {
                return None;
            }
            let label = line.split_whitespace().last()?;
            lines[..at].iter().any(|before| *before == format!("{label}:")).then(|| (at, label.to_string()))
        })
        .expect("a backward branch");
    let start = lines.iter().position(|line| *line == format!("{label}:")).unwrap();
    lines[start..=at].join("\n")
}

/// An `<8 x i64>` accumulator was one register (a 512-bit `q`): the phi had no
/// breakdown into `v2i64` parts and the function was refused.
#[test]
fn a_wide_vector_phi_is_four_q_registers_in_the_loop() {
    let text = assembly(
        "define void @f(ptr %d, ptr %p) {\n\
         entry:\n\
         \x20 br label %l\n\
         l:\n\
         \x20 %i = phi i64 [ 0, %entry ], [ %i2, %l ]\n\
         \x20 %a = phi <8 x i64> [ zeroinitializer, %entry ], [ %a2, %l ]\n\
         \x20 %q = getelementptr inbounds <8 x i64>, ptr %p, i64 %i\n\
         \x20 %x = load <8 x i64>, ptr %q, align 16\n\
         \x20 %a2 = add <8 x i64> %a, %x\n\
         \x20 %i2 = add nuw i64 %i, 1\n\
         \x20 %c = icmp eq i64 %i2, 100\n\
         \x20 br i1 %c, label %x1, label %l\n\
         x1:\n\
         \x20 store <8 x i64> %a2, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    let body = loop_body(&text);
    assert_eq!(count(&body, "add", ".2d"), 4, "{text}");
    assert_eq!(count(&body, "mov", " v") + count(&body, "fmov", " d"), 0, "accumulator copied in the loop:\n{text}");
    assert!(!body.contains("[sp"), "accumulator spilled in the loop:\n{text}");
    assert_eq!(count(&text, "stp", " q"), 2, "{text}");
}

/// The loop of `be1_tf`, its reduction replaced by a store: two `<16 x i64>`
/// accumulators of zero-extended bytes are sixteen `v2i64` parts that `uaddw`
/// and `uaddw2` add into, none moved or spilled.
#[test]
fn two_wide_accumulators_of_extended_bytes_are_uaddw_chains() {
    let text = assembly(
        "define void @f(ptr %d, ptr %p) {\n\
         entry:\n\
         \x20 br label %l\n\
         l:\n\
         \x20 %i = phi i64 [ 0, %entry ], [ %i2, %l ]\n\
         \x20 %a = phi <16 x i64> [ zeroinitializer, %entry ], [ %a2, %l ]\n\
         \x20 %b = phi <16 x i64> [ zeroinitializer, %entry ], [ %b2, %l ]\n\
         \x20 %q = getelementptr inbounds i8, ptr %p, i64 %i\n\
         \x20 %q2 = getelementptr inbounds i8, ptr %q, i64 16\n\
         \x20 %x = load <16 x i8>, ptr %q, align 1\n\
         \x20 %y = load <16 x i8>, ptr %q2, align 1\n\
         \x20 %xz = zext <16 x i8> %x to <16 x i64>\n\
         \x20 %yz = zext <16 x i8> %y to <16 x i64>\n\
         \x20 %a2 = add <16 x i64> %a, %xz\n\
         \x20 %b2 = add <16 x i64> %b, %yz\n\
         \x20 %i2 = add nuw i64 %i, 32\n\
         \x20 %c = icmp eq i64 %i2, 4000\n\
         \x20 br i1 %c, label %x1, label %l\n\
         x1:\n\
         \x20 %s = add <16 x i64> %b2, %a2\n\
         \x20 store <16 x i64> %s, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    let body = loop_body(&text);
    assert_eq!(count_dest(&body, "uaddw", ".2d"), 16, "{text}");
    assert_eq!(count(&body, "mov", " v") + count(&body, "fmov", " d"), 0, "accumulator copied in the loop:\n{text}");
    assert!(!body.contains("[sp"), "accumulator spilled in the loop:\n{text}");
}

/// The loop of `be1_mulred`, its reduction replaced by a store: the product of
/// two sign-extended `<8 x i16>` is `smull`/`smull2` on the narrow operands
/// (`performVectorExtCombine`, `LowerMUL`), widened by `saddw`.
#[test]
fn a_wide_product_accumulator_is_smull_and_saddw() {
    let text = assembly(
        "define void @f(ptr %d, ptr %p, ptr %q) {\n\
         entry:\n\
         \x20 br label %l\n\
         l:\n\
         \x20 %i = phi i64 [ 0, %entry ], [ %i2, %l ]\n\
         \x20 %a = phi <8 x i64> [ zeroinitializer, %entry ], [ %a2, %l ]\n\
         \x20 %pp = getelementptr inbounds i16, ptr %p, i64 %i\n\
         \x20 %qq = getelementptr inbounds i16, ptr %q, i64 %i\n\
         \x20 %x = load <8 x i16>, ptr %pp, align 2\n\
         \x20 %y = load <8 x i16>, ptr %qq, align 2\n\
         \x20 %xs = sext <8 x i16> %x to <8 x i64>\n\
         \x20 %ys = sext <8 x i16> %y to <8 x i64>\n\
         \x20 %m = mul nsw <8 x i64> %xs, %ys\n\
         \x20 %a2 = add <8 x i64> %a, %m\n\
         \x20 %i2 = add nuw i64 %i, 8\n\
         \x20 %c = icmp eq i64 %i2, 800\n\
         \x20 br i1 %c, label %x1, label %l\n\
         x1:\n\
         \x20 store <8 x i64> %a2, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    let body = loop_body(&text);
    assert_eq!(count_dest(&body, "smull", ".4s"), 2, "{text}");
    assert_eq!(count_dest(&body, "saddw", ".2d"), 4, "{text}");
    assert!(!body.contains("[sp"), "accumulator spilled in the loop:\n{text}");
}

/// `zext <16 x i8> to <16 x i64>` was refused: it is two `ushll .8h`, four
/// `.4s` and eight `.2d` (`SplitVecRes_ExtendOp` doubles the lane a step at a
/// time).
#[test]
fn a_byte_vector_zero_extended_to_i64_is_a_chain_of_ushll() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <16 x i8>, ptr %a, align 16\n\
         \x20 %z = zext <16 x i8> %x to <16 x i64>\n\
         \x20 store <16 x i64> %z, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    let shifts = |suffix| count_dest(&text, "ushll", suffix);
    assert_eq!((shifts(".8h"), shifts(".4s"), shifts(".2d")), (2, 4, 8), "{text}");
    assert_eq!(count(&text, "umov", " "), 0, "{text}");
}

/// `sext <8 x i16> to <8 x i64>`: two `sshll .4s` and four `.2d`.
#[test]
fn a_halfword_vector_sign_extended_to_i64_is_a_chain_of_sshll() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <8 x i16>, ptr %a, align 16\n\
         \x20 %z = sext <8 x i16> %x to <8 x i64>\n\
         \x20 store <8 x i64> %z, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    let shifts = |suffix| count_dest(&text, "sshll", suffix);
    assert_eq!((shifts(".4s"), shifts(".2d")), (2, 4), "{text}");
}

/// `add <16 x i64>` was refused: eight `add .2d`.
#[test]
fn a_1024_bit_add_is_eight_registers() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a, ptr %b) {\n\
         \x20 %x = load <16 x i64>, ptr %a, align 16\n\
         \x20 %y = load <16 x i64>, ptr %b, align 16\n\
         \x20 %z = add <16 x i64> %x, %y\n\
         \x20 store <16 x i64> %z, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "add", ".2d"), 8, "{text}");
}

/// NEON has no 64-bit lane multiply: `mul <4 x i64>` of unextended operands is
/// four scalar multiplies (`madd`, `UnrollVectorOp`), joined by `ins`.
#[test]
fn a_vector_i64_multiply_is_scalar_multiplies() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a, ptr %b) {\n\
         \x20 %x = load <4 x i64>, ptr %a, align 16\n\
         \x20 %y = load <4 x i64>, ptr %b, align 16\n\
         \x20 %z = mul <4 x i64> %x, %y\n\
         \x20 store <4 x i64> %z, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "madd", " x"), 4, "{text}");
    assert_eq!(count(&text, "mul", ".2d"), 0, "{text}");
}

/// A `v2i64` multiply of two sign-extended `<2 x i32>` is one `smull .2d`.
#[test]
fn a_product_of_extended_words_is_smull() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a, ptr %b) {\n\
         \x20 %x = load <2 x i32>, ptr %a, align 8\n\
         \x20 %y = load <2 x i32>, ptr %b, align 8\n\
         \x20 %xs = sext <2 x i32> %x to <2 x i64>\n\
         \x20 %ys = sext <2 x i32> %y to <2 x i64>\n\
         \x20 %z = mul <2 x i64> %xs, %ys\n\
         \x20 store <2 x i64> %z, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count_dest(&text, "smull", ".2d"), 1, "{text}");
    assert_eq!(count(&text, "madd", " x"), 0, "{text}");
}

/// A `v2i64` multiply by a splat of `2^n + 1` is a shift and an add
/// (`convertMulToShlAdd`).
#[test]
fn a_vector_i64_multiply_by_33_is_shift_and_add() {
    let text = assembly(
        "define void @f(ptr %d, ptr %a) {\n\
         \x20 %x = load <2 x i64>, ptr %a, align 16\n\
         \x20 %z = mul <2 x i64> %x, <i64 33, i64 33>\n\
         \x20 store <2 x i64> %z, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert_eq!(count(&text, "shl", ".2d"), 1, "{text}");
    assert_eq!(count(&text, "add", ".2d"), 1, "{text}");
    assert_eq!(count(&text, "madd", " x"), 0, "{text}");
}
