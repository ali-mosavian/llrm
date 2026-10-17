//! Every `KnownBits` operation against LLVM's own, on every input of widths 1
//! to 5 and a sample of width 8: `tools/llvmport/knownbits_dump.sh` runs LLVM
//! `d1106deb71cc` over the same inputs in the same order and hashes the
//! answers; a line of the golden file is one operation at one width.

use crate::knownbits::{KnownBits, rot_sign_bits};

struct Hash(u64);

impl Hash {
    fn mix(
        &mut self,
        v: u64,
    ) {
        for i in 0..8 {
            self.0 ^= (v >> (8 * i)) & 0xff;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn out(
        &mut self,
        k: &KnownBits,
    ) {
        self.mix(u64::from(k.width));
        self.mix(k.zero as u64);
        self.mix(k.one as u64);
    }

    fn line(
        &mut self,
        lines: &mut Vec<String>,
        name: &str,
        width: u32,
    ) {
        lines.push(format!("{name} {width} {:016x}", self.0));
        self.0 = 0xcbf29ce484222325;
    }
}

fn make(
    width: u32,
    zero: u64,
    one: u64,
) -> KnownBits {
    KnownBits::from_masks(width, u128::from(zero), u128::from(one))
}

fn inputs(width: u32) -> Vec<KnownBits> {
    let mut v = Vec::new();
    if width <= 5 {
        for c in 0..3_u64.pow(width) {
            let (mut zero, mut one, mut t) = (0_u64, 0_u64, c);
            for i in 0..width {
                match t % 3 {
                    1 => zero |= 1 << i,
                    2 => one |= 1 << i,
                    _ => {}
                }
                t /= 3;
            }
            v.push(make(width, zero, one));
        }
    } else {
        let mut s = 0x9e3779b97f4a7c15_u64;
        let mut next = || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            s >> 33
        };
        for _ in 0..160 {
            let (a, b, keep) = (next() & 0xff, next() & 0xff, next() & 0xff);
            v.push(make(width, a & keep & !b, b & keep));
        }
        v.push(make(width, 0, 0));
        v.push(make(width, 0xff, 0));
        v.push(make(width, 0, 0xff));
    }
    v
}

type Bin = Box<dyn Fn(&KnownBits, &KnownBits) -> KnownBits>;
type Cmp = fn(&KnownBits, &KnownBits) -> Option<bool>;
type Un = Box<dyn Fn(&KnownBits) -> KnownBits>;

fn binaries() -> Vec<(String, Bin)> {
    let mut v: Vec<(String, Bin)> = Vec::new();
    for nsw in [false, true] {
        for nuw in [false, true] {
            let s = format!("{}{}", u8::from(nsw), u8::from(nuw));
            v.push((format!("add{s}"), Box::new(move |l, r| KnownBits::add(l, r, nsw, nuw, false))));
            v.push((format!("sub{s}"), Box::new(move |l, r| KnownBits::sub(l, r, nsw, nuw))));
        }
    }
    let mut b = |name: &str, f: Bin| v.push((name.to_owned(), f));
    b("sadd_sat", Box::new(KnownBits::sadd_sat));
    b("ssub_sat", Box::new(KnownBits::ssub_sat));
    b("uadd_sat", Box::new(KnownBits::uadd_sat));
    b("usub_sat", Box::new(KnownBits::usub_sat));
    b("avgFloorS", Box::new(KnownBits::avg_floor_s));
    b("avgFloorU", Box::new(KnownBits::avg_floor_u));
    b("avgCeilS", Box::new(KnownBits::avg_ceil_s));
    b("avgCeilU", Box::new(KnownBits::avg_ceil_u));
    b("mul", Box::new(|l, r| KnownBits::mul(l, r, false)));
    b("mulhs", Box::new(KnownBits::mulhs));
    b("mulhu", Box::new(KnownBits::mulhu));
    b("sdiv", Box::new(|l, r| KnownBits::sdiv(l, r, false)));
    b("sdivExact", Box::new(|l, r| KnownBits::sdiv(l, r, true)));
    b("udiv", Box::new(|l, r| KnownBits::udiv(l, r, false)));
    b("udivExact", Box::new(|l, r| KnownBits::udiv(l, r, true)));
    b("urem", Box::new(KnownBits::urem));
    b("srem", Box::new(KnownBits::srem));
    b("umax", Box::new(KnownBits::umax));
    b("umin", Box::new(KnownBits::umin));
    b("smax", Box::new(KnownBits::smax));
    b("smin", Box::new(KnownBits::smin));
    b("abdu", Box::new(KnownBits::abdu));
    b("abds", Box::new(|l, r| KnownBits::abds(*l, *r)));
    b("shl", Box::new(|l, r| KnownBits::shl(l, r, false, false, false)));
    b("shlNuw", Box::new(|l, r| KnownBits::shl(l, r, true, false, false)));
    b("shlNsw", Box::new(|l, r| KnownBits::shl(l, r, false, true, false)));
    b("shlNuwNsw", Box::new(|l, r| KnownBits::shl(l, r, true, true, false)));
    b("shlNonZero", Box::new(|l, r| KnownBits::shl(l, r, false, false, true)));
    b("lshr", Box::new(|l, r| KnownBits::lshr(l, r, false, false)));
    b("lshrNonZero", Box::new(|l, r| KnownBits::lshr(l, r, true, false)));
    b("lshrExact", Box::new(|l, r| KnownBits::lshr(l, r, false, true)));
    b("lshrNonZeroExact", Box::new(|l, r| KnownBits::lshr(l, r, true, true)));
    b("ashr", Box::new(|l, r| KnownBits::ashr(l, r, false, false)));
    b("ashrNonZero", Box::new(|l, r| KnownBits::ashr(l, r, true, false)));
    b("ashrExact", Box::new(|l, r| KnownBits::ashr(l, r, false, true)));
    b("ashrNonZeroExact", Box::new(|l, r| KnownBits::ashr(l, r, true, true)));
    b("and", Box::new(|l, r| l.and(r)));
    b("or", Box::new(|l, r| l.or(r)));
    b("xor", Box::new(|l, r| l.xor(r)));
    b("clmul", Box::new(KnownBits::clmul));
    b("pext", Box::new(KnownBits::pext));
    b("pdep", Box::new(KnownBits::pdep));
    b("intersect", Box::new(|l, r| l.intersect_with(r)));
    b("union", Box::new(|l, r| l.union_with(r)));
    v
}

fn comparisons() -> Vec<(&'static str, Cmp)> {
    vec![
        ("eq", KnownBits::eq),
        ("ne", KnownBits::ne),
        ("ugt", KnownBits::ugt),
        ("uge", KnownBits::uge),
        ("ult", KnownBits::ult),
        ("ule", KnownBits::ule),
        ("sgt", KnownBits::sgt),
        ("sge", KnownBits::sge),
        ("slt", KnownBits::slt),
        ("sle", KnownBits::sle),
    ]
}

fn unaries() -> Vec<(String, Un)> {
    let mut v: Vec<(String, Un)> = Vec::new();
    let mut u = |name: &str, f: Un| v.push((name.to_owned(), f));
    u("abs", Box::new(|k| k.abs(false)));
    u("absPoison", Box::new(|k| k.abs(true)));
    u("reduceAdd3", Box::new(|k| k.reduce_add(3)));
    u("reduceAdd1", Box::new(|k| k.reduce_add(1)));
    u("blsi", Box::new(KnownBits::blsi));
    u("blsmsk", Box::new(KnownBits::blsmsk));
    u("reverseBits", Box::new(KnownBits::reverse_bits));
    u("anyext", Box::new(|k| k.anyext(k.width + 3)));
    u("zext", Box::new(|k| k.zext(k.width + 3)));
    u("sext", Box::new(|k| k.sext(k.width + 3)));
    for d in 1..5_u32 {
        u(&format!("trunc{d}"), Box::new(move |k| if d < k.width { k.trunc(d) } else { *k }));
        u(&format!("truncSSat{d}"), Box::new(move |k| if d < k.width { k.trunc_ssat(d) } else { *k }));
        u(&format!("truncSSatU{d}"), Box::new(move |k| if d < k.width { k.trunc_ssat_u(d) } else { *k }));
        u(&format!("truncUSat{d}"), Box::new(move |k| if d < k.width { k.trunc_usat(d) } else { *k }));
        u(&format!("sextInReg{d}"), Box::new(move |k| if d <= k.width { k.sext_in_reg(d) } else { *k }));
        u(&format!("extract{d}"), Box::new(move |k| if d < k.width { k.extract_bits(d, k.width - d) } else { *k }));
    }
    v
}

fn golden_lines() -> Vec<String> {
    let mut h = Hash(0xcbf29ce484222325);
    let mut lines = Vec::new();
    let (bins, cmps, uns) = (binaries(), comparisons(), unaries());
    for width in [1_u32, 2, 3, 4, 5, 8] {
        let input = inputs(width);
        for (name, f) in &bins {
            if (name == "mulhs" || name == "mulhu") && 2 * width > 8 {
                continue;
            }
            for l in &input {
                for r in &input {
                    h.out(&f(l, r));
                }
            }
            h.line(&mut lines, name, width);
        }
        for (name, f) in &cmps {
            for l in &input {
                for r in &input {
                    h.mix(f(l, r).map_or(2, u64::from));
                }
            }
            h.line(&mut lines, name, width);
        }
        for (name, f) in &uns {
            for k in &input {
                h.out(&f(k));
            }
            h.line(&mut lines, name, width);
        }
        for amount in 0..2 * width {
            for l in &input {
                for r in &input {
                    h.out(&KnownBits::fshl(l, r, u128::from(amount)));
                    h.out(&KnownBits::fshr(l, r, u128::from(amount)));
                }
            }
        }
        h.line(&mut lines, "funnel", width);
        for v in 0..(1_u128 << width).min(256) {
            for k in &input {
                h.out(&k.make_ge(v));
            }
        }
        h.line(&mut lines, "makeGE", width);
        for l in &input {
            for r in &input {
                for c in 0..3 {
                    let carry = make(1, u64::from(c == 1), u64::from(c == 2));
                    h.out(&KnownBits::compute_for_add_carry(l, r, &carry));
                    h.out(&KnownBits::compute_for_sub_borrow(l, *r, &carry));
                }
            }
        }
        h.line(&mut lines, "carry", width);
        for k in &input {
            for v in [
                k.count_min_trailing_zeros(),
                k.count_min_trailing_ones(),
                k.count_min_leading_zeros(),
                k.count_min_leading_ones(),
                k.count_min_sign_bits(),
                k.count_max_significant_bits(),
                k.count_max_trailing_zeros(),
                k.count_max_trailing_ones(),
                k.count_max_leading_zeros(),
                k.count_max_leading_ones(),
                k.count_min_population(),
                k.count_max_population(),
                k.count_max_active_bits(),
            ] {
                h.mix(u64::from(v));
            }
            for v in [k.min_value(), k.max_value(), k.signed_min_value(), k.signed_max_value()] {
                h.mix(v as u64);
            }
            for v in [k.is_non_zero(), k.is_strictly_positive(), k.is_non_positive(), k.is_sign_unknown(), k.is_even()]
            {
                h.mix(u64::from(v));
            }
        }
        h.line(&mut lines, "queries", width);
    }
    lines
}

/// Each operation's answers agree with LLVM's over every input: a line that
/// differs names the operation and the width.
#[test]
fn test_every_operation_agrees_with_llvm() {
    let golden: Vec<&str> = include_str!("knownbits_golden.txt").lines().collect();
    let ours = golden_lines();
    let differing: Vec<String> =
        ours.iter().zip(&golden).filter(|(a, b)| a.as_str() != **b).map(|(a, b)| format!("{a} (LLVM: {b})")).collect();
    assert_eq!(ours.len(), golden.len(), "{} operations here, {} in LLVM's file", ours.len(), golden.len());
    assert!(differing.is_empty(), "{} differ:\n{}", differing.len(), differing.join("\n"));
}

/// Known bits print as LLVM prints them.
#[test]
fn test_print_marks_unknown_known_and_conflicting_bits() {
    let mut k = make(4, 0b0101, 0b1000);
    assert_eq!(k.print(), "10?0");
    k.one |= 1;
    assert_eq!(k.print(), "10?!");
}

/// `rot` keeps the sign bits a rotate leaves, as LLVM's `SignBitsOps::rot`.
#[test]
fn test_rotate_keeps_the_sign_bits_it_does_not_rotate_out() {
    assert_eq!(rot_sign_bits(8, 8, Some(3), false), 8);
    assert_eq!(rot_sign_bits(5, 8, None, false), 1);
    assert_eq!(rot_sign_bits(5, 8, Some(2), false), 3);
    assert_eq!(rot_sign_bits(5, 8, Some(2), true), 1);
}
