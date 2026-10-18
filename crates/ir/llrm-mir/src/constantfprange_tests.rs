//! Every `ConstantFpRange` operation against LLVM's own, over every range of
//! the two 8-bit formats (the operations on one range) and the ranges between
//! 18 key values of every format (the operations on two):
//! `tools/llvmport/constantfprange_dump.sh` runs LLVM `d1106deb71cc` over the
//! same ranges in the same order and hashes the answers; a line of the golden
//! file is one operation on one format, with how many different answers it
//! gave.

use crate::apfloat::*;
use crate::constantfprange::{ConstantFpRange as R, strict_compare};
use crate::fpclass::{DenormalModeKind, FpClassTest};
use crate::golden_dump::{Dump, ROUNDINGS, add_f, assert_agree, assert_many_answers, formats, inputs, key_patterns};
use crate::opcode::FloatPredicate;

/// `FCmpInst::Predicate`, in LLVM's order.
const PREDS: [FloatPredicate; 16] = [
    FloatPredicate::False,
    FloatPredicate::Oeq,
    FloatPredicate::Ogt,
    FloatPredicate::Oge,
    FloatPredicate::Olt,
    FloatPredicate::Ole,
    FloatPredicate::One,
    FloatPredicate::Ord,
    FloatPredicate::Uno,
    FloatPredicate::Ueq,
    FloatPredicate::Ugt,
    FloatPredicate::Uge,
    FloatPredicate::Ult,
    FloatPredicate::Ule,
    FloatPredicate::Une,
    FloatPredicate::True,
];

const KINDS: [DenormalModeKind; 4] =
    [DenormalModeKind::Ieee, DenormalModeKind::PreserveSign, DenormalModeKind::PositiveZero, DenormalModeKind::Dynamic];

fn add_r(
    d: &mut Dump,
    r: &R,
) {
    add_f(d, r.get_lower());
    add_f(d, r.get_upper());
    d.add(u64::from(r.contains_qnan()));
    d.add(u64::from(r.contains_snan()));
}

fn add_opt_r(
    d: &mut Dump,
    r: &Option<R>,
) {
    d.add(u64::from(r.is_some()));
    if let Some(r) = r {
        add_r(d, r);
    }
}

/// Every range whose bounds come from `values` (non-NaN), with the four NaN
/// flags: lower <= upper, then the canonical empty range [+inf, -inf].
fn ranges_of(
    sem: Sem,
    values: &[ApFloat],
) -> Vec<R> {
    let mut v = Vec::new();
    for flags in 0..4 {
        let (q, s) = (flags & 1 != 0, flags & 2 != 0);
        for lo in values {
            for hi in values {
                if strict_compare(lo, hi) != CmpResult::GreaterThan {
                    v.push(R::new(*lo, *hi, q, s));
                }
            }
        }
        v.push(R::new(ApFloat::get_inf(sem, false), ApFloat::get_inf(sem, true), q, s));
    }
    v
}

fn golden_lines() -> Vec<String> {
    let mut d = Dump::new();
    let fmts = formats();
    for f in &fmts {
        let n = f.name;
        let s = f.sem;
        let keys: Vec<ApFloat> = key_patterns(f).iter().map(|b| ApFloat::from_bits(s, *b)).collect();
        let bounds: Vec<ApFloat> = if f.tiny {
            inputs(f).iter().map(|b| ApFloat::from_bits(s, *b)).filter(|x| !x.is_nan()).collect()
        } else {
            keys.clone()
        };
        let unary_ranges = ranges_of(s, &bounds);
        let key_ranges = ranges_of(s, &keys);
        let mut probes = keys.clone();
        for neg in [false, true] {
            probes.push(ApFloat::get_qnan(s, neg, None));
            probes.push(ApFloat::get_snan(s, neg, None));
        }

        // Constants.
        d.begin();
        for a in [false, true] {
            for b in [false, true] {
                add_r(&mut d, &R::get_nan_only(s, a, b));
            }
        }
        add_r(&mut d, &R::get_empty(s));
        add_r(&mut d, &R::get_full(s));
        add_r(&mut d, &R::get_finite(s));
        add_r(&mut d, &R::get_non_nan(s));
        d.end();
        d.line("constants", Some(n));
        for b in inputs(f) {
            d.begin();
            add_r(&mut d, &R::from_value(&ApFloat::from_bits(s, b)));
            d.end();
        }
        d.line("fromValue", Some(n));

        let un = |d: &mut Dump, name: &str, f: &dyn Fn(&mut Dump, &R)| {
            for r in &unary_ranges {
                d.begin();
                f(d, r);
                d.end();
            }
            d.line(name, Some(n));
        };
        un(&mut d, "queries", &|d, r| {
            for b in [
                r.is_full_set(),
                r.is_empty_set(),
                r.is_nan_only(),
                r.contains_nan(),
                r.is_single_element(false),
                r.is_single_element(true),
            ] {
                d.add(u64::from(b));
            }
            for ex in [false, true] {
                let e = r.get_single_element(ex);
                d.add(u64::from(e.is_some()));
                if let Some(e) = e {
                    add_f(d, e);
                }
            }
            d.add(r.get_sign_bit().map_or(0, |sign| 1 + u64::from(sign)));
            let FpClassTest(class) = r.classify();
            d.add(u64::from(class));
            add_f(d, r.get_lower());
            add_f(d, r.get_upper());
        });
        un(&mut d, "contains", &|d, r| {
            for p in &probes {
                d.add(u64::from(r.contains(p)));
            }
        });
        un(&mut d, "abs", &|d, r| add_r(d, &r.abs()));
        un(&mut d, "negate", &|d, r| add_r(d, &r.negate()));
        un(&mut d, "getWithoutNaN", &|d, r| add_r(d, &r.get_without_nan()));
        un(&mut d, "getWithoutInf", &|d, r| add_r(d, &r.get_without_inf()));
        un(&mut d, "flushDenormals", &|d, r| {
            for k in KINDS {
                let mut t = *r;
                t.flush_denormals(k);
                add_r(d, &t);
            }
        });
        for g in &fmts {
            if g.name == f.name {
                continue;
            }
            un(&mut d, &format!("cast_{}", g.name), &|d, r| {
                for rm in ROUNDINGS {
                    add_r(d, &r.cast(g.sem, rm));
                }
            });
        }

        // Predicates against a range, over every range of the 8-bit formats.
        for (i, pred) in PREDS.iter().enumerate() {
            un(&mut d, &format!("makeAllowed{i}"), &|d, r| add_r(d, &R::make_allowed_fcmp_region(*pred, r)));
            un(&mut d, &format!("makeSatisfying{i}"), &|d, r| add_r(d, &R::make_satisfying_fcmp_region(*pred, r)));
        }
        let values = inputs(f);
        for (i, pred) in PREDS.iter().enumerate() {
            for b in &values {
                d.begin();
                add_opt_r(&mut d, &R::make_exact_fcmp_region(*pred, &ApFloat::from_bits(s, *b)));
                d.end();
            }
            d.line(&format!("makeExact{i}"), Some(n));
        }

        // Binary operations over the ranges between the key values.
        let bin = |d: &mut Dump, name: &str, f: &dyn Fn(&mut Dump, &R, &R)| {
            for a in &key_ranges {
                for b in &key_ranges {
                    d.begin();
                    f(d, a, b);
                    d.end();
                }
            }
            d.line(name, Some(n));
        };
        bin(&mut d, "intersectWith", &|d, a, b| add_r(d, &a.intersect_with(b)));
        bin(&mut d, "unionWith", &|d, a, b| add_r(d, &a.union_with(b)));
        bin(&mut d, "containsRange", &|d, a, b| {
            d.add(u64::from(a.contains_range(b)));
            d.add(u64::from(a == b));
            d.add(u64::from(a != b));
        });
        bin(&mut d, "add", &|d, a, b| add_r(d, &a.add(b)));
        bin(&mut d, "sub", &|d, a, b| add_r(d, &a.sub(b)));
        bin(&mut d, "mul", &|d, a, b| add_r(d, &a.mul(b)));
        bin(&mut d, "div", &|d, a, b| add_r(d, &a.div(b)));
        bin(&mut d, "fcmp", &|d, a, b| {
            for p in PREDS {
                d.add(u64::from(a.fcmp(p, b)));
            }
        });
    }
    d.lines
}

/// Each operation's answers agree with LLVM's over every range: a line that
/// differs names the operation and the format.
#[test]
fn test_every_operation_agrees_with_llvm() {
    assert_agree(&golden_lines(), include_str!("constantfprange_golden.txt"));
}

/// An oracle that can only say one thing proves nothing. The regions of the
/// predicates `false` and `true` are fixed sets by definition, so those lines
/// may have a single answer.
#[test]
fn the_pinned_llvm_answers_say_more_than_one_thing() {
    let golden = include_str!("constantfprange_golden.txt");
    let fixed: Vec<String> =
        ["makeAllowed0", "makeSatisfying15", "makeExact0", "makeExact15"].into_iter().map(str::to_owned).collect();
    let except: Vec<&str> = fixed.iter().map(String::as_str).chain(["constants"]).collect();
    assert_many_answers(golden, &except, 340);
}
