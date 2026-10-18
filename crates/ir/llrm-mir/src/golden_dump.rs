//! The hash the `tools/llvmport/*_dump.cpp` drivers print: each call of an
//! operation hashes its answers, a line of a golden file is the hash of all
//! the calls of one operation and how many different answers they gave.

use std::collections::BTreeSet;

use crate::apfloat::*;
use crate::fpclass::RoundingMode;

pub(crate) struct Dump {
    hash: u64,
    current: u64,
    distinct: BTreeSet<u64>,
    pub(crate) lines: Vec<String>,
}

const OFFSET: u64 = 0xcbf29ce484222325;

fn fnv(
    state: &mut u64,
    v: u64,
) {
    for i in 0..8 {
        *state ^= (v >> (8 * i)) & 0xff;
        *state = state.wrapping_mul(0x100000001b3);
    }
}

impl Dump {
    pub(crate) fn new() -> Self {
        Self { hash: OFFSET, current: OFFSET, distinct: BTreeSet::new(), lines: Vec::new() }
    }

    pub(crate) fn begin(&mut self) {
        self.current = OFFSET;
    }

    pub(crate) fn add(
        &mut self,
        v: u64,
    ) {
        fnv(&mut self.current, v);
    }

    /// A string: its length, then its bytes.
    pub(crate) fn add_str(
        &mut self,
        text: &str,
    ) {
        self.add(text.len() as u64);
        for byte in text.bytes() {
            self.add(u64::from(byte));
        }
    }

    pub(crate) fn end(&mut self) {
        self.distinct.insert(self.current);
        fnv(&mut self.hash, self.current);
    }

    /// Ends the operation: its line, `name [format] hash distinct`.
    pub(crate) fn line(
        &mut self,
        name: &str,
        format: Option<&str>,
    ) {
        let format = format.map_or(String::new(), |one| format!(" {one}"));
        self.lines.push(format!("{name}{format} {:016x} {}", self.hash, self.distinct.len()));
        self.hash = OFFSET;
        self.distinct.clear();
    }
}

/// The lines of `ours` and of LLVM's file `golden` agree one by one.
pub(crate) fn assert_agree(
    ours: &[String],
    golden: &str,
) {
    let golden: Vec<&str> = golden.lines().collect();
    assert_eq!(ours.len(), golden.len(), "{} operations here, {} in LLVM's file", ours.len(), golden.len());
    let differing: Vec<String> =
        ours.iter().zip(&golden).filter(|(a, b)| a.as_str() != **b).map(|(a, b)| format!("{a} (LLVM: {b})")).collect();
    assert!(differing.is_empty(), "{} differ:\n{}", differing.len(), differing.join("\n"));
}

/// An oracle that can only say one thing proves nothing: every line of the
/// file but `except` has at least two different answers.
pub(crate) fn assert_many_answers(
    golden: &str,
    except: &[&str],
    least_lines: usize,
) {
    let mut constant = Vec::new();
    let mut count = 0;
    for line in golden.lines() {
        let words: Vec<&str> = line.split(' ').collect();
        let distinct: usize = words[words.len() - 1].parse().expect("a count");
        count += 1;
        if distinct < 2 && !except.contains(&words[0]) {
            constant.push(line);
        }
    }
    assert!(count >= least_lines, "{count} lines");
    assert!(constant.is_empty(), "these always answer the same thing: {constant:?}");
}

/// The formats and the values the floating-point dump drivers run on:
/// `tools/llvmport/fpinputs.h`.
pub(crate) struct Fmt {
    pub(crate) name: &'static str,
    pub(crate) sem: Sem,
    pub(crate) eb: u32,
    pub(crate) fb: u32,
    pub(crate) explicit: bool,
    pub(crate) size: u32,
    pub(crate) tiny: bool,
}

pub(crate) fn formats() -> Vec<Fmt> {
    let f = |name, sem, eb, fb, explicit, size, tiny| Fmt { name, sem, eb, fb, explicit, size, tiny };
    vec![
        f("e5m2", FLOAT8_E5M2, 5, 2, false, 8, true),
        f("e4m3", FLOAT8_E4M3, 4, 3, false, 8, true),
        f("single", IEEE_SINGLE, 8, 23, false, 32, false),
        f("double", IEEE_DOUBLE, 11, 52, false, 64, false),
        f("x87", X87_DOUBLE_EXTENDED, 15, 63, true, 80, false),
    ]
}

pub(crate) fn ones(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1_u128 << bits) - 1 }
}

pub(crate) fn pattern(
    f: &Fmt,
    sign: u32,
    exp: u128,
    frac: u128,
    int_bit: u32,
) -> u128 {
    let trailing = f.fb + u32::from(f.explicit);
    (u128::from(sign) << (f.size - 1)) | (exp << trailing) | (u128::from(int_bit) << f.fb) | frac
}

/// The structured edge values of a format (both signs); not for the 8-bit
/// formats, which use every value.
pub(crate) fn fixed_patterns(f: &Fmt) -> Vec<u128> {
    let mut v = Vec::new();
    let bias = (1_u128 << (f.eb - 1)) - 1;
    let top = ones(f.eb);
    let fr = ones(f.fb);
    let x = u32::from(f.explicit);
    let half = 1_u128 << (f.fb - 1);
    for s in 0..2 {
        v.push(pattern(f, s, 0, 0, 0));
        v.push(pattern(f, s, 0, 1, 0));
        v.push(pattern(f, s, 0, fr, 0));
        v.push(pattern(f, s, 0, half, 0));
        v.push(pattern(f, s, 1, 0, x));
        v.push(pattern(f, s, 1, 1, x));
        v.push(pattern(f, s, bias - 1, half, x));
        v.push(pattern(f, s, bias, 0, x));
        v.push(pattern(f, s, bias, 1, x));
        v.push(pattern(f, s, bias, half, x));
        v.push(pattern(f, s, bias + 1, 0, x));
        v.push(pattern(f, s, bias + 1, half, x));
        v.push(pattern(f, s, bias + u128::from(f.fb) - 1, 1, x));
        v.push(pattern(f, s, bias + u128::from(f.fb), 0, x));
        v.push(pattern(f, s, bias + u128::from(f.fb) + 1, 0, x));
        v.push(pattern(f, s, top - 1, fr - 1, x));
        v.push(pattern(f, s, top - 1, fr, x));
        v.push(pattern(f, s, top - 1, 0, x));
        v.push(pattern(f, s, top, 0, x));
        v.push(pattern(f, s, top, half, x));
        v.push(pattern(f, s, top, half | 1, x));
        v.push(pattern(f, s, top, 1, x));
        v.push(pattern(f, s, top, 1 << (f.fb - 2), x));
        v.push(pattern(f, s, top, fr, x));
        if f.explicit {
            v.push(pattern(f, s, 0, 0, 1));
            v.push(pattern(f, s, 0, 1, 1));
            v.push(pattern(f, s, 1, 5, 0));
            v.push(pattern(f, s, bias, 0, 0));
            v.push(pattern(f, s, top, 0, 0));
            v.push(pattern(f, s, top, 3, 0));
        }
    }
    v
}

/// The values the dump drivers run on.
pub(crate) fn inputs(f: &Fmt) -> Vec<u128> {
    if f.tiny {
        return (0..256).collect();
    }
    let mut v = fixed_patterns(f);
    let bias = (1_u128 << (f.eb - 1)) - 1;
    let top = ones(f.eb);
    let fr = ones(f.fb);
    let mut st = 0x9e3779b97f4a7c15_u64 ^ u64::from(f.size);
    let mut next = || {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        st >> 33
    };
    for _ in 0..120 {
        let s = (next() & 1) as u32;
        let e: u128 = if next() & 1 == 1 {
            let d = (next() % 25) as i64 - 12;
            let c = bias as i64 + d;
            if c < 0 {
                0
            } else if c as u128 > top {
                top
            } else {
                c as u128
            }
        } else {
            u128::from(next()) % (top + 1)
        };
        let (w1, w2, w3) = (u128::from(next()), u128::from(next()), u128::from(next()));
        let mut frac = ((((w1 << 32) ^ w2) << 32) ^ w3) & fr;
        if next() % 4 == 0 {
            frac &= !ones(f.fb / 2);
        }
        let mut ib = if f.explicit { u32::from(e != 0) } else { 0 };
        if f.explicit && next() % 16 == 0 {
            ib ^= 1;
        }
        v.push(pattern(f, s, e, frac, ib));
    }
    v
}

/// The 18 values the range tests build ranges from, in increasing order (-0
/// is below +0): `keyPatterns` of `tools/llvmport/fpinputs.h`.
pub(crate) fn key_patterns(f: &Fmt) -> Vec<u128> {
    let bias = (1_u128 << (f.eb - 1)) - 1;
    let top = ones(f.eb);
    let fr = ones(f.fb);
    let half = 1_u128 << (f.fb - 1);
    let x = u32::from(f.explicit);
    vec![
        pattern(f, 1, top, 0, x),
        pattern(f, 1, top - 1, fr, x),
        pattern(f, 1, bias + 1, 0, x),
        pattern(f, 1, bias, 0, x),
        pattern(f, 1, 1, 0, x),
        pattern(f, 1, 0, fr, 0),
        pattern(f, 1, 0, 1, 0),
        pattern(f, 1, 0, 0, 0),
        pattern(f, 0, 0, 0, 0),
        pattern(f, 0, 0, 1, 0),
        pattern(f, 0, 0, fr, 0),
        pattern(f, 0, 1, 0, x),
        pattern(f, 0, bias - 1, half, x),
        pattern(f, 0, bias, 0, x),
        pattern(f, 0, bias, half, x),
        pattern(f, 0, bias + 1, 0, x),
        pattern(f, 0, top - 1, fr, x),
        pattern(f, 0, top, 0, x),
    ]
}

/// The rounding modes in the order the drivers run them.
pub(crate) const ROUNDINGS: [RoundingMode; 5] = [
    RoundingMode::TowardZero,
    RoundingMode::NearestTiesToEven,
    RoundingMode::TowardPositive,
    RoundingMode::TowardNegative,
    RoundingMode::NearestTiesToAway,
];

/// A float as the drivers hash it: its bits, low word then high.
pub(crate) fn add_f(
    d: &mut Dump,
    f: &ApFloat,
) {
    let bits = f.to_bits();
    d.add(bits as u64);
    d.add((bits >> 64) as u64);
}
