//! Every `KnownFpClass` operation against LLVM's own, over every class mask
//! with the three sign states, eight denormal modes and a sample of the values
//! of every format: `tools/llvmport/knownfpclass_dump.sh` runs LLVM
//! `d1106deb71cc` over the same inputs in the same order and hashes the
//! answers; a line of the golden file is one operation, with how many
//! different answers it gave.

use crate::apfloat::*;
use crate::fpclass::*;
use crate::golden_dump::{Dump, assert_agree, assert_many_answers, formats, inputs, ones};
use crate::knownbits::KnownBits;
use crate::knownfpclass::{KnownFpClass, MinMaxKind};

type States = Vec<KnownFpClass>;

fn add_k(
    d: &mut Dump,
    k: &KnownFpClass,
) {
    d.add(u64::from(k.get_known_fp_classes().0));
    d.add(k.get_sign_bit().map_or(0, |sign| 1 + u64::from(sign)));
}

fn add_bits(
    d: &mut Dump,
    k: &KnownBits,
) {
    d.add(u64::from(k.width));
    for p in [k.zero, k.one] {
        d.add(p as u64);
        d.add(if k.width > 64 { (p >> 64) as u64 } else { 0 });
    }
}

fn sign_of(sg: u32) -> Option<bool> {
    if sg == 0 { None } else { Some(sg == 2) }
}

fn all_states() -> States {
    let mut s = Vec::new();
    for m in 0..1024 {
        for sg in 0..3 {
            s.push(KnownFpClass::new(FpClassTest(m), sign_of(sg)));
        }
    }
    s
}

fn full_states() -> States {
    (0..1024).map(|m| KnownFpClass::new(FpClassTest(m), None)).collect()
}

fn curated_masks() -> Vec<u32> {
    let mut v = vec![
        0,
        1,
        2,
        4,
        8,
        16,
        32,
        64,
        128,
        256,
        512,
        FC_NAN.0,
        FC_INF.0,
        FC_NORMAL.0,
        FC_SUBNORMAL.0,
        FC_ZERO.0,
        FC_POS_FINITE.0,
        FC_NEG_FINITE.0,
        FC_FINITE.0,
        FC_POSITIVE.0,
        FC_NEGATIVE.0,
        FC_ALL_FLAGS.0,
        (FC_ALL_FLAGS & !FC_NAN).0,
        (FC_ALL_FLAGS & !FC_ZERO).0,
        (FC_ALL_FLAGS & !FC_INF).0,
        (FC_ZERO | FC_NAN).0,
        (FC_POS_ZERO | FC_POS_NORMAL).0,
        (FC_NEG_ZERO | FC_NEG_NORMAL).0,
    ];
    let mut st = 0x2545f4914f6cdd1d_u64;
    for _ in 0..24 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((st >> 33) & 0x3ff) as u32);
    }
    v
}

fn curated_states(signs: bool) -> States {
    let mut s = Vec::new();
    for m in curated_masks() {
        for sg in 0..if signs { 3 } else { 1 } {
            s.push(KnownFpClass::new(FpClassTest(m), sign_of(sg)));
        }
    }
    s
}

fn all_modes() -> Vec<DenormalMode> {
    use DenormalModeKind::*;
    vec![
        DenormalMode::new(Ieee, Ieee),
        DenormalMode::new(PreserveSign, PreserveSign),
        DenormalMode::new(PositiveZero, PositiveZero),
        DenormalMode::new(Dynamic, Dynamic),
        DenormalMode::new(Ieee, PreserveSign),
        DenormalMode::new(PositiveZero, Ieee),
        DenormalMode::new(Dynamic, Ieee),
        DenormalMode::new(PreserveSign, Dynamic),
    ]
}

fn two_modes() -> Vec<DenormalMode> {
    vec![DenormalMode::get_ieee(), DenormalMode::get_dynamic()]
}

/// The known bits of a pattern: the bits of `mask` are known.
fn known_of(
    width: u32,
    v: u128,
    mask: u128,
) -> KnownBits {
    KnownBits::from_masks(width, !v & mask, v & mask)
}

fn exp_count(width: u32) -> u32 {
    if width > 5 { 300 } else { 3_u32.pow(width) }
}

/// The known bits of an exponent: every state of a width up to 5, else a fixed
/// sample.
fn exp_bits(
    width: u32,
    c: u32,
    st: &mut u64,
) -> KnownBits {
    let (mut z, mut o) = (0_u64, 0_u64);
    if width <= 5 {
        let mut t = c;
        for i in 0..width {
            match t % 3 {
                1 => z |= 1 << i,
                2 => o |= 1 << i,
                _ => {}
            }
            t /= 3;
        }
    } else {
        let m = (1_u64 << width) - 1;
        let mut draw = || {
            *st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (*st >> 33) & m
        };
        let a = draw();
        let b = draw();
        let keep = draw();
        z = a & keep & !b & m;
        o = b & keep;
    }
    KnownBits::from_masks(width, u128::from(z), u128::from(o))
}

fn golden_lines() -> Vec<String> {
    let mut d = Dump::new();
    let all = all_states();
    let full = full_states();
    let cur = curated_states(true);
    let cur_no_sign = curated_states(false);
    let modes = all_modes();
    let two = two_modes();
    let fmts = formats();
    let masks = curated_masks();

    // State queries.
    for k in &all {
        d.begin();
        for b in [
            k.is_known_never_nan(),
            k.is_known_always_nan(),
            k.is_known_never_infinity(),
            k.is_known_never_inf_or_nan(),
            k.is_known_never_pos_infinity(),
            k.is_known_never_neg_infinity(),
            k.is_known_never_subnormal(),
            k.is_known_never_pos_subnormal(),
            k.is_known_never_neg_subnormal(),
            k.is_known_never_zero(),
            k.is_known_never_pos_zero(),
            k.is_known_never_neg_zero(),
            k.cannot_be_ordered_less_than_zero(),
            k.cannot_be_ordered_greater_than_zero(),
            k.sign_bit_is_zero_or_nan(),
            k.is_unknown(),
        ] {
            d.add(u64::from(b));
        }
        for m in &masks {
            d.add(u64::from(k.is_known_never(FpClassTest(*m))));
            d.add(u64::from(k.is_known_always(FpClassTest(*m))));
        }
        d.end();
    }
    d.line("queries", None);
    for k in &all {
        for m in &modes {
            d.begin();
            d.add(u64::from(k.is_known_never_logical_zero(*m)));
            d.add(u64::from(k.is_known_never_logical_neg_zero(*m)));
            d.add(u64::from(k.is_known_never_logical_pos_zero(*m)));
            d.add(u64::from(k.cannot_be_ordered_less_eq_zero(*m)));
            d.add(u64::from(k.cannot_be_ordered_greater_eq_zero(*m)));
            d.end();
        }
    }
    d.line("logicalQueries", None);

    // Mutators and unary operations of the state alone.
    let un = |d: &mut Dump, name: &str, f: &dyn Fn(&mut Dump, &KnownFpClass)| {
        for k in &all {
            d.begin();
            f(d, k);
            d.end();
        }
        d.line(name, None);
    };
    un(&mut d, "knownNot", &|d, k| {
        for m in &masks {
            let mut t = *k;
            t.known_not(FpClassTest(*m));
            add_k(d, &t);
        }
    });
    un(&mut d, "fneg", &|d, k| {
        let mut t = *k;
        t.fneg();
        add_k(d, &t);
        add_k(d, &KnownFpClass::fneg_of(k));
    });
    un(&mut d, "fabs", &|d, k| {
        let mut t = *k;
        t.fabs();
        add_k(d, &t);
        add_k(d, &KnownFpClass::fabs_of(k));
    });
    un(&mut d, "signBit", &|d, k| {
        let (mut a, mut b, mut c) = (*k, *k, *k);
        a.sign_bit_must_be_zero();
        b.sign_bit_must_be_one();
        c.reset_all();
        add_k(d, &a);
        add_k(d, &b);
        add_k(d, &c);
    });
    un(&mut d, "propagateNonNaN", &|d, k| {
        let (mut a, mut b) = (KnownFpClass::default(), KnownFpClass::default());
        a.propagate_non_snan(k);
        b.propagate_non_nan(k);
        add_k(d, &a);
        add_k(d, &b);
    });
    type Unary = fn(&KnownFpClass) -> KnownFpClass;
    let unaries: [(&str, Unary); 11] = [
        ("exp", KnownFpClass::exp),
        ("sin", KnownFpClass::sin),
        ("cos", KnownFpClass::cos),
        ("tan", KnownFpClass::tan),
        ("sinh", KnownFpClass::sinh),
        ("cosh", KnownFpClass::cosh),
        ("tanh", KnownFpClass::tanh),
        ("asin", KnownFpClass::asin),
        ("acos", KnownFpClass::acos),
        ("atan", KnownFpClass::atan),
        ("fptrunc", KnownFpClass::fptrunc),
    ];
    for (name, f) in &unaries {
        un(&mut d, name, &|d, k| add_k(d, &f(k)));
    }
    un(&mut d, "roundToIntegral", &|d, k| {
        for t in [false, true] {
            for mu in [false, true] {
                add_k(d, &KnownFpClass::round_to_integral(k, t, mu));
            }
        }
    });
    un(&mut d, "fpext", &|d, k| {
        for dst in &fmts {
            for src in &fmts {
                add_k(d, &KnownFpClass::fpext(k, dst.sem, src.sem));
            }
        }
    });
    un(&mut d, "toKnownBits", &|d, k| {
        for f in &fmts {
            add_bits(d, &k.to_known_bits(f.sem));
        }
    });

    // Operations of one state and a mode.
    let um = |d: &mut Dump, name: &str, f: &dyn Fn(&mut Dump, &KnownFpClass, DenormalMode)| {
        for k in &all {
            for m in &modes {
                d.begin();
                f(d, k, *m);
                d.end();
            }
        }
        d.line(name, None);
    };
    um(&mut d, "fadd_self", &|d, k, m| add_k(d, &KnownFpClass::fadd_self(k, m)));
    um(&mut d, "fdiv_self", &|d, k, m| add_k(d, &KnownFpClass::fdiv_self(k, m)));
    um(&mut d, "frem_self", &|d, k, m| add_k(d, &KnownFpClass::frem_self(k, m)));
    um(&mut d, "square", &|d, k, m| add_k(d, &KnownFpClass::square(k, m)));
    um(&mut d, "canonicalize", &|d, k, m| add_k(d, &KnownFpClass::canonicalize(k, m)));
    um(&mut d, "log", &|d, k, m| add_k(d, &KnownFpClass::log(k, m)));
    um(&mut d, "sqrt", &|d, k, m| add_k(d, &KnownFpClass::sqrt(k, m)));
    um(&mut d, "frexp_mant", &|d, k, m| add_k(d, &KnownFpClass::frexp_mant(k, m)));
    um(&mut d, "propagateDenormal", &|d, k, m| {
        let mut a = KnownFpClass::new(FC_NONE, Some(true));
        let mut b = KnownFpClass::default();
        a.propagate_denormal(k, m);
        b.propagate_canonicalizing_src(k, m);
        add_k(d, &a);
        add_k(d, &b);
    });

    // Binary operations over every mask (sign unknown) with two modes, and over
    // the curated states with every mode.
    let bin = |d: &mut Dump, name: &str, f: &dyn Fn(&mut Dump, &KnownFpClass, &KnownFpClass, DenormalMode)| {
        for pass in 0..2 {
            let s = if pass == 0 { &full } else { &cur };
            let ms = if pass == 0 { &two } else { &modes };
            for a in s {
                for b in s {
                    for m in ms {
                        d.begin();
                        f(d, a, b, *m);
                        d.end();
                    }
                }
            }
            d.line(&format!("{name}{}", if pass == 0 { ".full" } else { ".cur" }), None);
        }
    };
    bin(&mut d, "fadd", &|d, a, b, m| add_k(d, &KnownFpClass::fadd(a, b, m)));
    bin(&mut d, "fsub", &|d, a, b, m| add_k(d, &KnownFpClass::fsub(a, b, m)));
    bin(&mut d, "fmul", &|d, a, b, m| add_k(d, &KnownFpClass::fmul(a, b, m)));
    bin(&mut d, "fdiv", &|d, a, b, m| add_k(d, &KnownFpClass::fdiv(a, b, m)));
    bin(&mut d, "frem", &|d, a, b, m| add_k(d, &KnownFpClass::frem(a, b, m)));
    bin(&mut d, "atan2", &|d, a, b, m| add_k(d, &KnownFpClass::atan2(a, b, m)));
    bin(&mut d, "fma_square", &|d, a, b, m| add_k(d, &KnownFpClass::fma_square(a, b, m)));
    bin(&mut d, "minMaxLike", &|d, a, b, m| {
        for kind in [
            MinMaxKind::Minimum,
            MinMaxKind::Maximum,
            MinMaxKind::Minimumnum,
            MinMaxKind::Maximumnum,
            MinMaxKind::Minnum,
            MinMaxKind::Maxnum,
        ] {
            add_k(d, &KnownFpClass::min_max_like(a, b, kind, m));
        }
    });
    bin(&mut d, "pairs", &|d, a, b, _| {
        add_k(d, &KnownFpClass::pow(a, b));
        add_k(d, &a.intersect_with(b));
        add_k(d, &a.union_with(b));
        add_k(d, &(*a | *b));
        let mut t = *a;
        t |= *b;
        add_k(d, &t);
        add_k(d, &KnownFpClass::copysign_of(a, b));
        let mut c = *a;
        c.copysign(b);
        add_k(d, &c);
        let (mut x, mut y, mut z) = (KnownFpClass::default(), KnownFpClass::default(), KnownFpClass::default());
        x.propagate_non_snan2(a, b);
        y.propagate_non_nan2(a, b);
        z.propagate_xor_sign(a, b);
        add_k(d, &x);
        add_k(d, &y);
        add_k(d, &z);
        d.add(u64::from(a == b));
    });

    // fma over the curated masks.
    for a in &cur_no_sign {
        for b in &cur_no_sign {
            for c in &cur_no_sign {
                for m in &two {
                    d.begin();
                    add_k(&mut d, &KnownFpClass::fma(a, b, c, *m));
                    d.end();
                }
            }
        }
    }
    d.line("fma", None);

    // Constants.
    for f in &fmts {
        for bits in inputs(f) {
            d.begin();
            add_k(&mut d, &KnownFpClass::from_apfloat(&ApFloat::from_bits(f.sem, bits)));
            d.end();
        }
        d.line(&format!("fromAPFloat.{}", f.name), None);
    }
    for f in &fmts {
        let values = inputs(f);
        for k in &cur {
            for bits in &values {
                for m in &two {
                    d.begin();
                    add_k(&mut d, &KnownFpClass::fmul_apfloat(k, &ApFloat::from_bits(f.sem, *bits), *m));
                    d.end();
                }
            }
        }
        d.line(&format!("fmulConstant.{}", f.name), None);
    }

    // Bitcast and known bits.
    for f in &fmts {
        let mut kb: Vec<KnownBits> = Vec::new();
        let exp_mask = ones(f.size - 1) & !ones(f.fb + u32::from(f.explicit));
        let quiet = 1_u128 << (f.fb - 1);
        let sign = 1_u128 << (f.size - 1);
        let man_mask = ones(f.fb);
        let mut st = 0x51ed270b0b1d4f3d_u64 ^ u64::from(f.size);
        let mut next = || {
            st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            st >> 33
        };
        for v in inputs(f) {
            let r1 = u128::from(next());
            let r2 = u128::from(next());
            let r1 = (r1 << 32) ^ r2;
            for mk in [ones(f.size), sign, exp_mask, exp_mask | quiet, man_mask, 0, sign | exp_mask, r1 & ones(f.size)]
            {
                kb.push(known_of(f.size, v, mk));
            }
        }
        for i in 0..200 {
            let (a, b, c) = (u128::from(next()), u128::from(next()), u128::from(next()));
            let (e, g, h) = (u128::from(next()), u128::from(next()), u128::from(next()));
            let za = (((a << 32) ^ b) << 16) ^ c;
            let oa = (((e << 32) ^ g) << 16) ^ h;
            let (z, o) = if i % 4 == 0 { (za, oa) } else { (za & !oa, oa) };
            kb.push(KnownBits::from_masks(f.size, z & ones(f.size), o & ones(f.size)));
        }
        kb.push(known_of(f.size, 0, 0));
        for k in &kb {
            d.begin();
            add_k(&mut d, &KnownFpClass::bitcast(f.sem, k));
            d.end();
        }
        d.line(&format!("bitcast.{}", f.name), None);
    }

    // ldexp.
    let exponents: [i64; 18] = [-300, -24, -23, -1, 0, 1, 2, 3, 22, 23, 24, 51, 52, 53, 62, 63, 64, 300];
    for f in &fmts {
        for k in &cur {
            for lo in exponents {
                for hi in exponents {
                    if lo > hi {
                        continue;
                    }
                    for m in &two {
                        d.begin();
                        add_k(&mut d, &KnownFpClass::ldexp(k, i128::from(lo), i128::from(hi), f.sem, *m));
                        d.end();
                    }
                }
            }
        }
        d.line(&format!("ldexp.{}", f.name), None);
    }
    for w in [5_u32, 8] {
        for f in &fmts {
            for k in &cur {
                let mut st = 0x3c6ef372fe94f82b_u64;
                for c in 0..exp_count(w) {
                    let e = exp_bits(w, c, &mut st);
                    for m in &two {
                        d.begin();
                        add_k(&mut d, &KnownFpClass::ldexp_known_bits(k, &e, f.sem, *m));
                        d.end();
                    }
                }
            }
            d.line(&format!("ldexpBits{w}.{}", f.name), None);
        }
    }

    // powi.
    for w in [4_u32, 8] {
        for k in &cur {
            let mut st = 0x7a1c3d5e9b2f4601_u64;
            for c in 0..if w == 4 { exp_count(4) } else { 300 } {
                d.begin();
                add_k(&mut d, &KnownFpClass::powi(k, &exp_bits(w, c, &mut st)));
                d.end();
            }
        }
        d.line(&format!("powi{w}"), None);
    }
    d.lines
}

/// Each operation's answers agree with LLVM's over every state: a line that
/// differs names the operation.
#[test]
fn test_every_operation_agrees_with_llvm() {
    assert_agree(&golden_lines(), include_str!("knownfpclass_golden.txt"));
}

/// An oracle that can only say one thing proves nothing. The 8-bit test
/// formats are ones LLVM has not validated `bitcast` for: it answers unknown.
#[test]
fn the_pinned_llvm_answers_say_more_than_one_thing() {
    assert_many_answers(include_str!("knownfpclass_golden.txt"), &["bitcast.e5m2", "bitcast.e4m3"], 70);
}
