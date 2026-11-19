//! `computeKnownBits` against what LLVM answered on the same IR: the lines of
//! `tests/vt/known/NAME.expected` are `tools/llvmport/vtdump.sh kb,nn,ng
//! NAME.ll`.

use super::{AssumptionCache, Query, column};
use crate::datalayout::DataLayout;
use crate::domtree::DomTree;
use crate::module::{GlobalKind, InstId, Operand};
use crate::parse;

/// What `examples/vtdump.rs` prints for `kb,nn,ng` of every integer and pointer
/// value in `text`.
fn dump(text: &str) -> String {
    let module = parse::module(text).expect("parses");
    let layout =
        module.datalayout.as_deref().map(|text| DataLayout::parse(text).expect("a layout")).unwrap_or_default();
    let mut out = String::new();
    for global in &module.globals {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().unwrap_or_default();
        let dt = DomTree::of(function);
        let assumptions = AssumptionCache::new(&module, function);
        let first = function.block(function.entry().expect("an entry")).first();
        let mut values: Vec<(Operand, Option<InstId>)> =
            function.parameters().iter().map(|parameter| (Operand::Value(*parameter), first)).collect();
        for (_, inst) in function.walk() {
            if let Some(result) = function.instruction(inst).result {
                values.push((Operand::Value(result), Some(inst)));
            }
        }
        let mut n = 0;
        for (value, cxt) in values {
            let mut query = Query::new(&module, function, &layout).with_cxt(cxt);
            query.dt = Some(&dt);
            query.assumptions = Some(&assumptions);
            if !(1..=128).contains(&query.scalar_bits(value)) {
                continue;
            }
            out.push_str(&format!("{name} {n}"));
            n += 1;
            for col in ["kb", "nn", "ng"] {
                out.push_str(&column(&query, value, col));
            }
            out.push('\n');
        }
    }
    out
}

fn assert_same(
    got: &str,
    want: &str,
) {
    let (got, want): (Vec<_>, Vec<_>) = (got.lines().collect(), want.lines().collect());
    assert_eq!(got.len(), want.len(), "values compared");
    for (got, want) in got.iter().zip(&want) {
        assert_eq!(got, want);
    }
}

/// `and x, -x` and `xor x, x-1` keep the lowest set bit's mask; and/or/xor
/// with `x + odd` fix bit 0; the `a*(b-c)+c*d` pattern bounds the sum.
#[test]
fn test_arithmetic_and_bit_idioms_match_llvm() {
    assert_same(&dump(include_str!("../../tests/vt/known/ops.ll")), include_str!("../../tests/vt/known/ops.expected"));
}

/// Recurrences, phi merges and the range a branch gives an incoming value.
#[test]
fn test_phi_recurrences_match_llvm() {
    assert_same(
        &dump(include_str!("../../tests/vt/known/phis.ll")),
        include_str!("../../tests/vt/known/phis.expected"),
    );
}

/// A `getelementptr` adds its indices to the base's known low bits, at the
/// index width of its address space.
#[test]
fn test_getelementptr_matches_llvm() {
    assert_same(&dump(include_str!("../../tests/vt/known/gep.ll")), include_str!("../../tests/vt/known/gep.expected"));
}

/// `range` attributes and `!range`, `!align`, `align`, `returned`, and the
/// alignment of allocas and globals.
#[test]
fn test_ranges_and_alignment_match_llvm() {
    assert_same(
        &dump(include_str!("../../tests/vt/known/attrs.ll")),
        include_str!("../../tests/vt/known/attrs.expected"),
    );
}

/// The intrinsics `computeKnownBitsFromOperator` knows.
#[test]
fn test_intrinsics_match_llvm() {
    assert_same(
        &dump(include_str!("../../tests/vt/known/intrinsics.ll")),
        include_str!("../../tests/vt/known/intrinsics.expected"),
    );
}

/// A constant expression is an operator: `sdiv i1 ptrtoint (ptr @g to i1), ..`
/// had no known bits, where LLVM knows `@g` is aligned and the quotient 0.
#[test]
fn test_constant_expressions_are_operators() {
    assert_same(
        &dump(include_str!("../../tests/vt/known/constexpr.ll")),
        include_str!("../../tests/vt/known/constexpr.expected"),
    );
}

/// A call to `llvm.cttz` with one argument made `computeKnownBits` index past
/// its arguments and panic; it answers nothing instead.
#[test]
fn test_intrinsic_call_with_missing_argument_is_unknown() {
    let text = "declare i32 @llvm.cttz.i32(i32)\ndefine i32 @f(i32 %x) {\n  %r = call i32 @llvm.cttz.i32(i32 %x)\n  ret i32 %r\n}\n";
    assert_same(
        &dump(text),
        "f 0 kb=???????????????????????????????? nn=0 ng=0\nf 1 kb=???????????????????????????????? nn=0 ng=0\n",
    );
}

/// `computeKnownBits` of `undef` says nothing, and a phi whose incoming values
/// are all undef is skipped, as in LLVM.
#[test]
fn test_undef_operands_match_llvm() {
    assert_same(
        &dump(include_str!("../../tests/vt/known/undef.ll")),
        include_str!("../../tests/vt/known/undef.expected"),
    );
}

/// Each operand kind `computeKnownBits` reads before the operator: an integer
/// constant, a null pointer, undef, poison, a `range` argument and an aligned
/// pointer argument keep their bits when the kind is read once.
#[test]
fn test_operand_kinds_have_their_known_bits() {
    let text = "declare void @use(i8, ptr, i8, i8, i8, ptr)\n\
        define void @f(i8 range(i8 0, 16) %r, ptr align 8 %p) {\n  \
        call void @use(i8 5, ptr null, i8 undef, i8 poison, i8 %r, ptr %p)\n  ret void\n}\n";
    let module = parse::module(text).expect("parses");
    let layout = DataLayout::default();
    let function = module
        .globals
        .iter()
        .find_map(|global| match &global.kind {
            GlobalKind::Function(function) if !function.is_declaration() => Some(function),
            _ => None,
        })
        .expect("@f");
    let q = Query::new(&module, function, &layout);
    let (_, call) = function.walk().next().expect("the call");
    let got: Vec<String> = function.instruction(call).operands[..6]
        .iter()
        .map(|&operand| super::known::compute_known_bits(&q, operand, 0).print())
        .collect();
    let pointer = |low: &str| format!("{}{low}", "?".repeat(got[1].len() - low.len()));
    assert_eq!(got[0], "00000101");
    assert_eq!(got[1], "0".repeat(got[1].len()));
    assert_eq!(got[2], "????????");
    assert_eq!(got[3], "????????");
    assert_eq!(got[4], "0000????");
    assert_eq!(got[5], pointer("000"));
}

/// `and` of two plain adds (bit 0 unknown) looked for `x op (x +/- y)` through
/// closure matchers that read each operand up to six times, 126 Ir a miss;
/// `low_bit_partner` reads each once. Counts `instruction_of` calls for the
/// whole query of `%c`.
#[test]
fn a_miss_of_the_low_bit_partner_reads_each_operand_once() {
    let module = parse::module(
        "define i32 @f(i32 %p, i32 %q, i32 %r, i32 %s) {\n  %a = add i32 %p, %q\n  %b = add i32 %r, %s\n  %c = and i32 %a, %b\n  ret i32 %c\n}\n",
    )
    .expect("parses");
    let layout = DataLayout::default();
    let function = module
        .globals
        .iter()
        .find_map(|global| match &global.kind {
            GlobalKind::Function(function) if !function.is_declaration() => Some(function),
            _ => None,
        })
        .expect("a body");
    let c = function.walk().filter_map(|(_, inst)| function.instruction(inst).result).last().expect("a result");
    let query = Query::new(&module, function, &layout);
    super::query::LOOKUPS.with(|count| count.set(0));
    let known = super::known::compute_known_bits(&query, Operand::Value(c), 0);
    let lookups = super::query::LOOKUPS.with(|count| count.get());
    assert_eq!(known.width, 32);
    assert!(lookups <= LOOKUPS_BOUND, "{lookups} operand lookups for a miss of the low-bit partner");
}

const LOOKUPS_BOUND: usize = 7;
