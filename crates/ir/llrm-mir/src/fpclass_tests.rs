//! Every `FPClassTest` and `DenormalMode` operation against LLVM's own, on
//! every mask and every mode: `tools/llvmport/fpclass_dump.sh` runs LLVM
//! `d1106deb71cc` over the same inputs in the same order and hashes the
//! answers; a line of the golden file is one operation, with how many
//! different answers it gave.

use crate::fpclass::*;
use crate::golden_dump::{Dump, assert_agree, assert_many_answers};

fn unary(
    d: &mut Dump,
    name: &str,
    mut f: impl FnMut(&mut Dump, FpClassTest),
) {
    for m in 0..1024 {
        d.begin();
        f(d, FpClassTest(m));
        d.end();
    }
    d.line(name, None);
}

fn kind_number(kind: DenormalModeKind) -> u64 {
    (kind as i8 as i64 + 1) as u64
}

fn mode_numbers(
    d: &mut Dump,
    mode: DenormalMode,
) {
    d.add(kind_number(mode.output));
    d.add(kind_number(mode.input));
}

const KINDS: [DenormalModeKind; 5] = [
    DenormalModeKind::Invalid,
    DenormalModeKind::Ieee,
    DenormalModeKind::PreserveSign,
    DenormalModeKind::PositiveZero,
    DenormalModeKind::Dynamic,
];

fn golden_lines() -> Vec<String> {
    let mut d = Dump::new();
    unary(&mut d, "fneg", |d, m| d.add(u64::from(fneg(m).0)));
    unary(&mut d, "inverse_fabs", |d, m| d.add(u64::from(inverse_fabs(m).0)));
    unary(&mut d, "unknown_sign", |d, m| d.add(u64::from(unknown_sign(m).0)));
    unary(&mut d, "not", |d, m| d.add(u64::from((!m).0)));
    unary(&mut d, "print", |d, m| d.add_str(&print_fp_class_test(m)));
    for z in [false, true] {
        unary(&mut d, &format!("orderedStrictlyLess{}", u8::from(z)), |d, m| {
            d.add(u64::from(ordered_strictly_less(m, z).0));
        });
        unary(&mut d, &format!("orderedStrictlyGreater{}", u8::from(z)), |d, m| {
            d.add(u64::from(ordered_strictly_greater(m, z).0));
        });
    }
    for f in [false, true] {
        unary(&mut d, &format!("invertIfSimpler{}", u8::from(f)), |d, m| {
            d.add(u64::from(invert_fp_class_test_if_simpler(m, f).0));
        });
    }
    for z in [false, true] {
        type Order = fn(FpClassTest, FpClassTest, bool) -> bool;
        let ops: [(&str, Order); 4] = [
            ("cannotOrderStrictlyGreater", cannot_order_strictly_greater),
            ("cannotOrderStrictlyGreaterEq", cannot_order_strictly_greater_eq),
            ("cannotOrderStrictlyLess", cannot_order_strictly_less),
            ("cannotOrderStrictlyLessEq", cannot_order_strictly_less_eq),
        ];
        for (name, f) in ops {
            for l in 0..1024 {
                for r in 0..1024 {
                    d.begin();
                    d.add(u64::from(f(FpClassTest(l), FpClassTest(r), z)));
                    d.end();
                }
            }
            d.line(&format!("{name}{}", u8::from(z)), None);
        }
    }

    for r in -1..8_i8 {
        d.begin();
        let rounding = match r {
            0 => RoundingMode::TowardZero,
            1 => RoundingMode::NearestTiesToEven,
            2 => RoundingMode::TowardPositive,
            3 => RoundingMode::TowardNegative,
            4 => RoundingMode::NearestTiesToAway,
            7 => RoundingMode::Dynamic,
            _ => RoundingMode::Invalid,
        };
        d.add_str(spell(rounding));
        d.end();
    }
    d.line("spell", None);
    let mut modes = Vec::new();
    for output in KINDS {
        for input in KINDS {
            modes.push(DenormalMode::new(output, input));
        }
    }
    for m in &modes {
        d.begin();
        mode_numbers(&mut d, *m);
        for flag in [
            m.is_simple(),
            m.is_valid(),
            m.inputs_are_zero(),
            m.inputs_may_be_zero(),
            m.outputs_are_zero(),
            m.outputs_may_be_zero(),
            m.inputs_may_be_positive_zero(),
            m.outputs_may_be_positive_zero(),
        ] {
            d.add(u64::from(flag));
        }
        if m.is_valid() {
            d.add(u64::from(m.to_int_value()));
        }
        d.add_str(&m.str());
        for legacy in [false, true] {
            for omit in [false, true] {
                d.add_str(&m.print(legacy, omit));
            }
        }
        d.end();
    }
    d.line("denormalMode", None);
    for a in &modes {
        for b in &modes {
            d.begin();
            mode_numbers(&mut d, a.merge_callee_mode(*b));
            d.add(u64::from(a == b));
            d.add(u64::from(a != b));
            d.end();
        }
    }
    d.line("merge", None);
    for i in 0..16 {
        d.begin();
        mode_numbers(&mut d, DenormalMode::create_from_int_value(i));
        d.end();
    }
    d.line("createFromIntValue", None);
    let names = [
        "",
        "ieee",
        "preservesign",
        "preserve-sign",
        "positivezero",
        "positive-zero",
        "dynamic",
        "bogus",
        "ieee,ieee",
        "dynamic,ieee",
        "preserve-sign,positive-zero",
        "ieee,bogus",
        "bogus,ieee",
        ",",
        "ieee,",
    ];
    for name in names {
        d.begin();
        mode_numbers(&mut d, parse_denormal_fp_attribute(name));
        d.add(kind_number(parse_denormal_fp_attribute_component(name)));
        d.end();
    }
    d.line("parse", None);
    for kind in KINDS {
        for legacy in [false, true] {
            d.begin();
            d.add_str(denormal_mode_kind_name(kind, legacy));
            d.end();
        }
    }
    d.line("kindName", None);

    // `DenormalFpEnv` over the valid modes and the all-invalid float mode,
    // which takes the default's.
    let env_modes: Vec<DenormalMode> = modes
        .iter()
        .copied()
        .filter(|m| m.is_valid() || (m.output == DenormalModeKind::Invalid && m.input == DenormalModeKind::Invalid))
        .collect();
    for base in env_modes.iter().filter(|m| m.is_valid()) {
        for fm in &env_modes {
            d.begin();
            let e = DenormalFpEnv::new(*base, *fm);
            mode_numbers(&mut d, e.default_mode);
            mode_numbers(&mut d, e.f32_mode);
            d.add(u64::from(e.to_int_value()));
            d.add(u64::from(DenormalFpEnv::create_from_int_value(e.to_int_value()) == e));
            for omit in [false, true] {
                d.add_str(&e.print(omit));
            }
            let other = DenormalFpEnv::new(if fm.is_valid() { *fm } else { *base }, *base);
            let merged = e.merge_callee_mode(other);
            mode_numbers(&mut d, merged.default_mode);
            mode_numbers(&mut d, merged.f32_mode);
            d.end();
        }
    }
    d.line("denormalFPEnv", None);
    d.begin();
    d.add(u64::from(DenormalFpEnv::get_default().to_int_value()));
    d.end();
    d.line("envDefault", None);
    d.lines
}

/// Each operation's answers agree with LLVM's over every mask and mode: a line
/// that differs names the operation.
#[test]
fn test_every_operation_agrees_with_llvm() {
    assert_agree(&golden_lines(), include_str!("fpclass_golden.txt"));
}

/// An oracle that can only say one thing proves nothing.
#[test]
fn the_pinned_llvm_answers_say_more_than_one_thing() {
    assert_many_answers(include_str!("fpclass_golden.txt"), &["envDefault"], 20);
}
