//! `MachineCombiner` reassociation on arm64
//! (`TargetInstrInfo::getMachineCombinerPatterns` with
//! `AArch64InstrInfo::isAssociativeAndCommutative`): a chain of one integer
//! operation is rebuilt so the operand that arrives last joins last.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const HEADER: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn asm(body: &str) -> String {
    driver::assembly(&format!("{HEADER}{body}"), Platform::Linux, Level::O1).expect("lowers")
}

/// The longest chain of `mnemonics` instructions, any other writer ending one.
fn longest_chain(
    asm: &str,
    mnemonics: &[&str],
) -> usize {
    let mut depth = llrm_support::hash::HashMap::<String, usize>::default();
    let mut longest = 0;
    for line in asm.lines().map(str::trim) {
        let Some((op, operands)) = line.split_once(char::is_whitespace) else { continue };
        let regs: Vec<String> = operands
            .split(',')
            .map(|part| part.trim().trim_start_matches('[').to_string())
            .filter(|part| part.starts_with(['x', 'w']) && part[1..].chars().all(|c| c.is_ascii_digit()))
            .map(|part| part[1..].to_string())
            .collect();
        let Some((dest, sources)) = regs.split_first() else { continue };
        let reached = if mnemonics.contains(&op) {
            1 + sources.iter().map(|reg| depth.get(reg).copied().unwrap_or(0)).max().unwrap_or(0)
        } else {
            0
        };
        depth.insert(dest.clone(), reached);
        longest = longest.max(reached);
    }
    longest
}

/// matrix `sfm16_float_cmp` -O1: `acc += (a<b) + 2*(a==b) + ...` added the six
/// terms to the accumulator one after the other, a chain of 6 per trip where
/// the pin's `MachineCombiner` builds a tree 4 deep: 400 trips modelled at
/// 4,458 clocks for 2,858 (llvm-mca apple-m1 per block, unicorn trace).
#[test]
fn a_sum_of_six_terms_into_an_accumulator_is_a_tree() {
    let ir = "define i64 @f(ptr %0, i64 %1) {\n\
  %3 = icmp sgt i64 %1, 0\n  br i1 %3, label %6, label %4\n\
4:\n  %5 = phi i64 [ 0, %2 ], [ %31, %6 ]\n  ret i64 %5\n\
6:\n  %7 = phi i64 [ %31, %6 ], [ 0, %2 ]\n  %8 = phi i64 [ %32, %6 ], [ 0, %2 ]\n\
  %9 = shl nuw nsw i64 %8, 3\n  %10 = getelementptr inbounds nuw i8, ptr %0, i64 %9\n\
  %11 = load float, ptr %10, align 4\n  %12 = getelementptr inbounds nuw i8, ptr %10, i64 4\n\
  %13 = load float, ptr %12, align 4\n\
  %14 = fcmp olt float %11, %13\n  %15 = zext i1 %14 to i64\n\
  %16 = fcmp oeq float %11, %13\n  %17 = select i1 %16, i64 2, i64 0\n\
  %18 = fcmp ogt float %11, %13\n  %19 = select i1 %18, i64 4, i64 0\n\
  %20 = fcmp une float %11, %13\n  %21 = select i1 %20, i64 8, i64 0\n\
  %22 = fcmp ole float %11, %13\n  %23 = select i1 %22, i64 16, i64 0\n\
  %24 = fcmp oge float %11, %13\n  %25 = select i1 %24, i64 32, i64 0\n\
  %26 = add i64 %7, %15\n  %27 = add i64 %26, %17\n  %28 = add i64 %27, %19\n\
  %29 = add i64 %28, %21\n  %30 = add i64 %29, %23\n  %31 = add i64 %30, %25\n\
  %32 = add nuw nsw i64 %8, 1\n  %33 = icmp eq i64 %32, %1\n  br i1 %33, label %4, label %6\n}\n";
    let text = asm(ir);
    assert!(longest_chain(&text, &["add", "cinc"]) <= 4, "{text}");
}

/// `acc | t1 | .. | t6` over six compares chained into one value six deep; the
/// pin's `MachineCombiner` makes it a tree 3 deep for `add`, `orr` and `eor`
/// alike (not `and`: the selects fold to one bit).
#[test]
fn a_chain_of_one_operation_over_an_argument_is_a_tree() {
    for (op, mnemonic) in [("add", "add"), ("or", "orr"), ("xor", "eor")] {
        let mut body = String::from("define i64 @f(float %a, float %b, i64 %acc) {\n");
        let mut prev = "%acc".to_string();
        for (n, predicate) in ["olt", "oeq", "ogt", "une", "ole", "oge"].iter().enumerate() {
            body += &format!(
                "  %c{n} = fcmp {predicate} float %a, %b\n  %t{n} = select i1 %c{n}, i64 {}, i64 1\n  %s{n} = {op} i64 {prev}, %t{n}\n",
                3 * (n + 1)
            );
            prev = format!("%s{n}");
        }
        body += &format!("  ret i64 {prev}\n}}\n");
        let text = asm(&body);
        assert!(longest_chain(&text, &[mnemonic]) <= 3, "{op}\n{text}");
    }
}
