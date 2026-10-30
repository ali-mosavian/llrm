//! Every `ApFloat` operation against LLVM's own, on every value of two 8-bit
//! formats and a sample of float, double and x87: `tools/llvmport/
//! apfloat_dump.sh` runs LLVM `d1106deb71cc` over the same inputs in the same
//! order and hashes the answers; a line of the golden file is one operation on
//! one format, with how many different answers it gave.

use crate::apfloat::*;
use crate::fpclass::RoundingMode;
use crate::golden_dump::{Dump, ROUNDINGS, add_f, assert_agree, assert_many_answers, formats, inputs};

fn each(
    d: &mut Dump,
    values: &[ApFloat],
    name: &str,
    format: &str,
    mut f: impl FnMut(&mut Dump, &ApFloat) -> bool,
) {
    for x in values {
        d.begin();
        if f(d, x) {
            d.end();
        }
    }
    d.line(name, Some(format));
}

fn pairs(
    d: &mut Dump,
    values: &[ApFloat],
    name: &str,
    format: &str,
    mut f: impl FnMut(&mut Dump, &ApFloat, &ApFloat) -> bool,
) {
    for x in values {
        for y in values {
            d.begin();
            if f(d, x, y) {
                d.end();
            }
        }
    }
    d.line(name, Some(format));
}

fn golden_lines() -> Vec<String> {
    let mut d = Dump::new();
    let fmts = formats();
    for f in &fmts {
        let bits = inputs(f);
        let v: Vec<ApFloat> = bits.iter().map(|b| ApFloat::from_bits(f.sem, *b)).collect();
        let n = f.name;
        let mut index = 0;
        each(&mut d, &v, "inputs", n, |d, _| {
            d.add(bits[index] as u64);
            d.add((bits[index] >> 64) as u64);
            index += 1;
            true
        });
        each(&mut d, &v, "bits", n, |d, x| {
            add_f(d, x);
            true
        });
        each(&mut d, &v, "flags", n, |d, x| {
            for flag in [
                x.is_nan(),
                x.is_infinity(),
                x.is_zero(),
                x.is_negative(),
                x.is_denormal(),
                x.is_normal(),
                x.is_smallest(),
                x.is_smallest_normalized(),
                x.is_largest(),
                x.is_finite(),
                x.is_finite_non_zero(),
                x.is_signaling(),
                x.is_integer(),
                x.is_non_zero(),
                x.is_pos_zero(),
                x.is_neg_zero(),
                x.is_pos_infinity(),
                x.is_neg_infinity(),
            ] {
                d.add(u64::from(flag));
            }
            d.add(x.get_category() as u64);
            true
        });
        each(&mut d, &v, "classify", n, |d, x| {
            d.add(u64::from(x.classify().0));
            true
        });
        each(&mut d, &v, "ilogb", n, |d, x| {
            d.add(x.ilogb() as i64 as u64);
            d.add(x.get_exact_log2_abs() as i64 as u64);
            d.add(x.get_exact_log2() as i64 as u64);
            for k in -3..4 {
                d.add(u64::from(x.is_power_of2(k)));
                d.add(u64::from(x.is_neg_power_of2(k)));
            }
            true
        });
        each(&mut d, &v, "nextUp", n, |d, x| {
            let mut y = *x;
            d.add(u64::from(y.next(false)));
            add_f(d, &y);
            true
        });
        each(&mut d, &v, "nextDown", n, |d, x| {
            let mut y = *x;
            d.add(u64::from(y.next(true)));
            add_f(d, &y);
            true
        });
        each(&mut d, &v, "neg", n, |d, x| {
            add_f(d, &x.neg());
            true
        });
        each(&mut d, &v, "abs", n, |d, x| {
            add_f(d, &x.abs());
            true
        });
        each(&mut d, &v, "makeQuiet", n, |d, x| {
            if !x.is_nan() {
                return false;
            }
            add_f(d, &x.make_quiet());
            true
        });
        for (r, rm) in ROUNDINGS.iter().enumerate() {
            each(&mut d, &v, &format!("roundToIntegral{r}"), n, |d, x| {
                let mut y = *x;
                d.add(u64::from(y.round_to_integral(*rm)));
                add_f(d, &y);
                true
            });
        }
        for g in &fmts {
            if g.name == f.name {
                continue;
            }
            for (r, rm) in ROUNDINGS.iter().enumerate() {
                each(&mut d, &v, &format!("convert_{}_{r}", g.name), n, |d, x| {
                    let mut y = *x;
                    let (status, loses) = y.convert(g.sem, *rm);
                    d.add(u64::from(status));
                    d.add(u64::from(loses));
                    add_f(d, &y);
                    true
                });
            }
        }

        // Constants and the queries on the semantics.
        d.begin();
        let s = f.sem;
        for neg in [false, true] {
            add_f(&mut d, &ApFloat::get_zero(s, neg));
            add_f(&mut d, &ApFloat::get_one(s, neg));
            add_f(&mut d, &ApFloat::get_inf(s, neg));
            add_f(&mut d, &ApFloat::get_largest(s, neg));
            add_f(&mut d, &ApFloat::get_smallest(s, neg));
            add_f(&mut d, &ApFloat::get_smallest_normalized(s, neg));
            for p in [0_u64, 1, 5, 0x123456789abcdef, !0] {
                add_f(&mut d, &ApFloat::get_nan(s, neg, p));
            }
            for p in [0_u64, 1, 2, 0x1234, !0] {
                add_f(&mut d, &ApFloat::get_qnan(s, neg, Some(u128::from(p))));
                add_f(&mut d, &ApFloat::get_snan(s, neg, Some(u128::from(p))));
            }
            add_f(&mut d, &ApFloat::get_qnan(s, neg, None));
            add_f(&mut d, &ApFloat::get_snan(s, neg, None));
        }
        d.end();
        d.line("constants", Some(n));
        d.begin();
        d.add(u64::from(semantics_precision(s)));
        d.add(semantics_max_exponent(s) as i64 as u64);
        d.add(semantics_min_exponent(s) as i64 as u64);
        d.add(u64::from(semantics_size_in_bits(s)));
        d.add(u64::from(get_size_in_bits(s)));
        d.add(u64::from(is_ieee_like_fp(s)));
        d.add(u64::from(semantics_has_inf(s)));
        d.add(u64::from(semantics_has_nan(s)));
        d.add(u64::from(semantics_has_signed_repr(s)));
        d.add(u64::from(semantics_has_zero(s)));
        for g in &fmts {
            d.add(u64::from(is_representable_as_normal_in(s, g.sem)));
        }
        d.end();
        d.line("semantics", Some(n));

        pairs(&mut d, &v, "compare", n, |d, x, y| {
            d.add(x.compare(y) as u64);
            true
        });
        pairs(&mut d, &v, "compareAbs", n, |d, x, y| {
            if !x.is_finite_non_zero() || !y.is_finite_non_zero() {
                return false;
            }
            d.add(x.compare_absolute_value(y) as u64);
            true
        });
        pairs(&mut d, &v, "bitwiseIsEqual", n, |d, x, y| {
            d.add(u64::from(x.bitwise_is_equal(y)));
            true
        });
        for (r, rm) in ROUNDINGS.iter().enumerate() {
            type Arith = fn(&mut ApFloat, &ApFloat, RoundingMode) -> OpStatus;
            let ops: [(&str, Arith); 4] = [
                ("add", |a, b, m| a.add(b, m)),
                ("sub", |a, b, m| a.subtract(b, m)),
                ("mul", |a, b, m| a.multiply(b, m)),
                ("div", |a, b, m| a.divide(b, m)),
            ];
            for (op, f) in ops {
                pairs(&mut d, &v, &format!("{op}{r}"), n, |d, x, y| {
                    let mut z = *x;
                    d.add(u64::from(f(&mut z, y, *rm)));
                    add_f(d, &z);
                    true
                });
            }
        }
        type Binary = fn(&ApFloat, &ApFloat) -> ApFloat;
        let binaries: [(&str, Binary); 11] = [
            ("plus", |x, y| x.plus(y)),
            ("minus", |x, y| x.minus(y)),
            ("times", |x, y| x.times(y)),
            ("dividedBy", |x, y| x.divided_by(y)),
            ("minnum", minnum),
            ("maxnum", maxnum),
            ("minimum", minimum),
            ("maximum", maximum),
            ("minimumnum", minimumnum),
            ("maximumnum", maximumnum),
            ("copySign", |x, y| {
                let mut z = *x;
                z.copy_sign(y);
                z
            }),
        ];
        for (name, op) in binaries {
            pairs(&mut d, &v, name, n, |d, x, y| {
                add_f(d, &op(x, y));
                true
            });
        }
    }
    d.lines
}

/// Each operation's answers agree with LLVM's over every input: a line that
/// differs names the operation and the format.
#[test]
fn test_every_operation_agrees_with_llvm() {
    assert_agree(&golden_lines(), include_str!("apfloat_golden.txt"));
}

/// An oracle that can only say one thing proves nothing: on every format,
/// most operations answer with at least two different things.
#[test]
fn the_pinned_llvm_answers_say_more_than_one_thing() {
    assert_many_answers(include_str!("apfloat_golden.txt"), &["constants", "semantics"], 300);
}

/// MIR has `float`, `double` and `x86_fp80` and nothing else, which is what
/// lets `ApFloat` leave out half, bfloat, quad and double-double.
/// `semantics_of` matches every `FloatKind`, so this stops compiling the day
/// MIR gains one; and the parser refuses the others.
#[test]
fn mir_has_only_the_float_kinds_apfloat_has() {
    for kind in [crate::FloatKind::Float, crate::FloatKind::Double, crate::FloatKind::X86Fp80] {
        let sem = semantics_of(kind);
        assert_eq!(sem.size_in_bits, crate::datalayout::float_bits(kind));
    }
    for ty in ["half", "bfloat", "fp128", "ppc_fp128"] {
        let text = format!("define {ty} @f({ty} %a) {{\n  ret {ty} %a\n}}\n");
        assert!(crate::parse::module(&text).is_err(), "{ty}");
    }
}
