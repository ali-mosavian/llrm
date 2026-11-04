//! `fp128` in MIR: its text forms, the constant it parses to and what the
//! passes that build constants make of it. The bit patterns are the pin's
//! `llc` output for `@g = constant fp128 <text>` (m68k, big-endian `.quad`
//! pairs: high word first).

use crate::context::ConstantKind;
use crate::module::{GlobalKind, Module, Operand};
use crate::parse;

/// The bits of `@g = constant fp128 <text>`.
fn bits(text: &str) -> u128 {
    let source = format!("@g = constant fp128 {text}\n");
    let module: Module = parse::module(&source).unwrap_or_else(|why| panic!("{why:?}\n{source}"));
    let GlobalKind::Variable(variable) = &module.globals[0].kind else { panic!("a variable") };
    match module.context.get(variable.initializer.expect("an initializer")).kind {
        ConstantKind::Float128(bits) => bits,
        ref other => panic!("{text}: {other:?}"),
    }
}

const fn quad(
    high: u64,
    low: u64,
) -> u128 {
    ((high as u128) << 64) | low as u128
}

/// Decimals the pin's clang prints for `long double`. Parsing one as a double
/// and widening it gave 0.1 = 0x3FFB999999999999A000000000000000.
#[test]
fn a_decimal_fp128_is_rounded_once_at_113_bits() {
    assert_eq!(bits("1.000000e+00"), quad(0x3fff_0000_0000_0000, 0));
    assert_eq!(bits("1.000000e-01"), quad(0x3ffb_9999_9999_9999, 0x9999_9999_9999_999a));
    assert_eq!(bits("-2.5"), quad(0xc000_4000_0000_0000, 0));
    assert_eq!(bits("3.14159265358979323846264338327950288"), quad(0x4000_921f_b544_42d1, 0x8469_898c_c517_01b8));
    assert_eq!(bits("1.0e4900"), quad(0x7f94_5d24_084e_b26f, 0x4981_5192_2505_cb5f));
    // The smallest denormal, from the digits the pin prints for it.
    assert_eq!(bits("6.475180e-4966"), 1);
    assert_eq!(bits("0.0"), 0);
    assert_eq!(bits("-0.0"), 1 << 127);
}

/// `0xL` and 32 hex digits: the LOW 64 bits first (the pin's `HexToIntPair`).
/// Read high-first, 1/3 would be a denormal.
#[test]
fn a_0xl_literal_is_the_low_half_first() {
    assert_eq!(bits("0xL55555555555555553FFD555555555555"), quad(0x3ffd_5555_5555_5555, 0x5555_5555_5555_5555));
    assert_eq!(bits("0xL00000000000000003FFF000000000000"), quad(0x3fff_0000_0000_0000, 0));
}

#[test]
fn f0x_and_the_special_values_are_the_same_bits() {
    assert_eq!(bits("f0x3FFF0000000000000000000000000000"), quad(0x3fff_0000_0000_0000, 0));
    assert_eq!(bits("+inf"), quad(0x7fff_0000_0000_0000, 0));
    assert_eq!(bits("-inf"), quad(0xffff_0000_0000_0000, 0));
    assert_eq!(bits("+qnan"), quad(0x7fff_8000_0000_0000, 0));
}

/// A finite value given as a double's `0x` digits is not an fp128 (LLVM's
/// "Half, BFloat, Float, and double only").
#[test]
fn a_double_hex_literal_is_not_an_fp128() {
    let why = parse::module("@g = constant fp128 0x3FF0000000000000\n").unwrap_err();
    assert!(format!("{why:?}").contains("fp128"), "{why:?}");
}

/// The printer's text reads back to the same constant, for halves that differ.
#[test]
fn an_fp128_prints_to_text_that_parses_to_itself() {
    for text in ["1.000000e-01", "0xL0123456789ABCDEF3FFB000000000001", "-inf", "6.475180e-4966"] {
        let source = format!("@g = constant fp128 {text}\n");
        let module = parse::module(&source).unwrap();
        let printed = crate::print::module(&module);
        let again = parse::module(&printed).unwrap_or_else(|why| panic!("{why:?}\n{printed}"));
        let (GlobalKind::Variable(a), GlobalKind::Variable(b)) = (&module.globals[0].kind, &again.globals[0].kind)
        else {
            panic!("variables")
        };
        assert_eq!(
            module.context.get(a.initializer.unwrap()).kind,
            again.context.get(b.initializer.unwrap()).kind,
            "{text} printed as\n{printed}"
        );
    }
}

/// `null_value` (and the other helpers that build a zero of any float type)
/// gave `Float(0)`, a 64-bit constant, on an fp128: its readers took the
/// type's width from the type and the bits from a double.
#[test]
fn the_zero_of_an_fp128_is_an_fp128_constant() {
    let module = parse::module("@g = constant fp128 1.0\n").unwrap();
    let ty = module
        .context
        .get(match &module.globals[0].kind {
            GlobalKind::Variable(variable) => variable.initializer.unwrap(),
            _ => panic!("a variable"),
        })
        .ty;
    let zero = crate::constfold::null_value(&module.context, ty);
    assert_eq!(module.context.get(zero).kind, ConstantKind::Float128(0));
}

/// `x - x` with no NaNs is +0.0, built through `constant_of`: it wrote
/// `Float(bits as u64)` on an fp128, a 64-bit constant of a 128-bit type.
#[test]
fn a_zero_simplify_builds_for_an_fp128_is_an_fp128_constant() {
    let text = "define fp128 @f(fp128 %x) {\nb0:\n  %r = fsub nnan fp128 %x, %x\n  ret fp128 %r\n}\n";
    let mut module: Module = parse::module(text).unwrap();
    let id = module.named("f").unwrap();
    let GlobalKind::Function(function) = &module.globals[id.0 as usize].kind else { panic!("a function") };
    let function = function.clone();
    let inst = function.walk().map(|(_, inst)| inst).next().unwrap();
    let found = crate::simplify::simplify_instruction_in(
        &mut module.context,
        &module.globals,
        &crate::datalayout::DataLayout::default(),
        None,
        &function,
        inst,
    );
    let Some(Operand::Constant(zero)) = found else { panic!("{found:?}") };
    assert_eq!(module.context.get(zero).kind, ConstantKind::Float128(0));
}

/// The interpreter evaluates floats as doubles: an fp128 module is refused,
/// not run.
#[test]
fn the_interpreter_refuses_an_fp128_module() {
    let text = "define fp128 @f(fp128 %x) {\nb0:\n  %r = fadd fp128 %x, 1.0\n  ret fp128 %r\n}\n";
    let module = parse::module(text).unwrap();
    let ran = crate::interpret::run(&module, "f", vec![], 100);
    assert!(matches!(ran, Err(crate::interpret::Trap::Unsupported(ref why)) if why.contains("fp128")), "{ran:?}");
}

/// `mangle` had no arm for fp128 and wrote its display name: the verifier
/// demanded `llvm.fabs.fp128`, where LLVM's `getMangledTypeStr` says `f128`.
#[test]
fn test_an_fp128_intrinsic_is_mangled_f128() {
    let module = parse::module("declare fp128 @llvm.fabs.f128(fp128)\n").unwrap_or_else(|why| panic!("{why:?}"));
    assert_eq!(crate::verify::verify(&module), Vec::<String>::new());
}
