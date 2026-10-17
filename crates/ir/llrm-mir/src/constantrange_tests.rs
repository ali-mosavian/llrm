//! Every `ConstantRange` operation against LLVM's own, on every range of widths
//! 1 to 4 and a sample of widths 6, 8, 16 and 64:
//! `tools/llvmport/constantrange_dump.sh` runs LLVM `d1106deb71cc` over the
//! same ranges in the same order and hashes the answers; a line of the golden
//! file is one operation at one width.

use crate::constantrange::{ConstantRange as CR, PreferredRangeType as Pref};
use crate::knownbits::KnownBits;
use crate::opcode::{BinaryOp, CastOp, IntPredicate};

const PREDICATES: [IntPredicate; 10] = [
    IntPredicate::Eq,
    IntPredicate::Ne,
    IntPredicate::Ugt,
    IntPredicate::Uge,
    IntPredicate::Ult,
    IntPredicate::Ule,
    IntPredicate::Sgt,
    IntPredicate::Sge,
    IntPredicate::Slt,
    IntPredicate::Sle,
];

fn predicate_number(p: IntPredicate) -> u64 {
    32 + PREDICATES.iter().position(|q| *q == p).unwrap() as u64
}

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
        r: &CR,
    ) {
        self.mix(u64::from(r.width));
        self.mix(r.lower as u64);
        self.mix(r.upper as u64);
    }

    fn out_known(
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

fn mask64(width: u32) -> u64 {
    if width == 64 { !0 } else { (1 << width) - 1 }
}

fn ranges(width: u32) -> Vec<CR> {
    let mut v = Vec::new();
    let m = mask64(width);
    if width <= 4 {
        for l in 0..=m {
            for u in 0..=m {
                if l == u && l != 0 && l != m {
                    continue;
                }
                v.push(CR::new(width, u128::from(l), u128::from(u)));
            }
        }
        return v;
    }
    let mut s = 0x9e3779b97f4a7c15_u64 ^ u64::from(width);
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (s >> 11) ^ (s << 20)
    };
    v.push(CR::full(width));
    v.push(CR::empty(width));
    let sm = 1_u64 << (width - 1);
    let specials = [
        0,
        1,
        2,
        3,
        m,
        m.wrapping_sub(1),
        m.wrapping_sub(2),
        sm,
        sm.wrapping_sub(1),
        sm.wrapping_add(1),
        sm.wrapping_add(2),
        sm.wrapping_sub(2),
    ];
    for mut a in specials {
        for b in [0, 1, sm, m, sm.wrapping_add(1), 5_u64] {
            a &= m;
            let b = b & m;
            if a == b && a != 0 && a != m {
                continue;
            }
            v.push(CR::new(width, u128::from(a), u128::from(b)));
        }
    }
    let n = if width <= 8 { 120 } else { 50 };
    for i in 0..n {
        let a = next() & m;
        let mut b = next() & m;
        if i % 3 == 0 {
            b = a.wrapping_add(next() & 0xf) & m;
        }
        if a == b && a != 0 && a != m {
            continue;
        }
        v.push(CR::new(width, u128::from(a), u128::from(b)));
    }
    v
}

type Bin = Box<dyn Fn(&CR, &CR) -> CR>;
type Un = Box<dyn Fn(&CR) -> CR>;

fn binaries() -> Vec<(String, Bin)> {
    let mut v: Vec<(String, Bin)> = Vec::new();
    let mut b = |name: &str, f: Bin| v.push((name.to_owned(), f));
    b("add", Box::new(|a, c| a.add(c)));
    b("sub", Box::new(|a, c| a.sub(c)));
    for k in 0..4_u32 {
        b(&format!("mul{k}"), Box::new(move |a, c| a.multiply(c, k)));
        b(&format!("addNW{k}"), Box::new(move |a, c| a.add_with_no_wrap(c, k, Pref::Smallest)));
        b(&format!("subNW{k}"), Box::new(move |a, c| a.sub_with_no_wrap(c, k, Pref::Smallest)));
        b(&format!("shlNW{k}"), Box::new(move |a, c| a.shl_with_no_wrap(c, k, Pref::Smallest)));
    }
    b("smul_fast", Box::new(|a, c| a.smul_fast(c)));
    b("smax", Box::new(|a, c| a.smax(c)));
    b("umax", Box::new(|a, c| a.umax(c)));
    b("smin", Box::new(|a, c| a.smin(c)));
    b("umin", Box::new(|a, c| a.umin(c)));
    b("udiv", Box::new(|a, c| a.udiv(c)));
    b("sdiv", Box::new(|a, c| a.sdiv(c)));
    b("urem", Box::new(|a, c| a.urem(c)));
    b("srem", Box::new(|a, c| a.srem(c)));
    b("and", Box::new(|a, c| a.binary_and(c)));
    b("or", Box::new(|a, c| a.binary_or(c, false)));
    b("orDisjoint", Box::new(|a, c| a.binary_or(c, true)));
    b("xor", Box::new(|a, c| a.binary_xor(c)));
    b("shl", Box::new(|a, c| a.shl(c)));
    b("lshr", Box::new(|a, c| a.lshr(c)));
    b("ashr", Box::new(|a, c| a.ashr(c)));
    b("uadd_sat", Box::new(|a, c| a.uadd_sat(c)));
    b("sadd_sat", Box::new(|a, c| a.sadd_sat(c)));
    b("usub_sat", Box::new(|a, c| a.usub_sat(c)));
    b("ssub_sat", Box::new(|a, c| a.ssub_sat(c)));
    b("umul_sat", Box::new(|a, c| a.umul_sat(c)));
    b("smul_sat", Box::new(|a, c| a.smul_sat(c)));
    b("ushl_sat", Box::new(|a, c| a.ushl_sat(c)));
    b("sshl_sat", Box::new(|a, c| a.sshl_sat(c)));
    b("difference", Box::new(|a, c| a.difference(c)));
    for (t, p) in [Pref::Smallest, Pref::Unsigned, Pref::Signed].into_iter().enumerate() {
        b(&format!("intersect{t}"), Box::new(move |a, c| a.intersect_with(c, p)));
        b(&format!("union{t}"), Box::new(move |a, c| a.union_with(c, p)));
    }
    b("addNW3signed", Box::new(|a, c| a.add_with_no_wrap(c, 3, Pref::Signed)));
    b("subNW3unsigned", Box::new(|a, c| a.sub_with_no_wrap(c, 3, Pref::Unsigned)));
    for (name, op, kind) in [
        ("noWrapAddU", BinaryOp::Add, 1),
        ("noWrapAddS", BinaryOp::Add, 2),
        ("noWrapSubU", BinaryOp::Sub, 1),
        ("noWrapSubS", BinaryOp::Sub, 2),
        ("noWrapMulU", BinaryOp::Mul, 1),
        ("noWrapMulS", BinaryOp::Mul, 2),
        ("noWrapShlU", BinaryOp::Shl, 1),
        ("noWrapShlS", BinaryOp::Shl, 2),
    ] {
        b(name, Box::new(move |_, c| CR::make_guaranteed_no_wrap_region(op, c, kind)));
    }
    for p in PREDICATES {
        let n = predicate_number(p);
        b(&format!("allowed{n}"), Box::new(move |_, c| CR::make_allowed_icmp_region(p, c)));
        b(&format!("satisfying{n}"), Box::new(move |_, c| CR::make_satisfying_icmp_region(p, c)));
        b(&format!("allowedSameSign{n}"), Box::new(move |_, c| CR::make_allowed_icmp_region_samesign(p, true, c)));
    }
    v
}

fn unaries() -> Vec<(String, Un)> {
    let mut v: Vec<(String, Un)> = Vec::new();
    let mut u = |name: &str, f: Un| v.push((name.to_owned(), f));
    u("inverse", Box::new(|a| a.inverse()));
    u("abs", Box::new(|a| a.abs(false)));
    u("absPoison", Box::new(|a| a.abs(true)));
    u("ctlz", Box::new(|a| a.ctlz(false)));
    u("ctlzPoison", Box::new(|a| a.ctlz(true)));
    u("cttz", Box::new(|a| a.cttz(false)));
    u("cttzPoison", Box::new(|a| a.cttz(true)));
    u("ctpop", Box::new(|a| a.ctpop()));
    u("sqrtFloor", Box::new(|a| a.sqrt_floor()));
    u("binaryNot", Box::new(|a| a.binary_not()));
    u("zext3", Box::new(|a| if a.width + 3 > 64 { *a } else { a.zero_extend(a.width + 3) }));
    u("sext3", Box::new(|a| if a.width + 3 > 64 { *a } else { a.sign_extend(a.width + 3) }));
    u("splitPos", Box::new(|a| a.split_pos_neg().0));
    u("splitNeg", Box::new(|a| a.split_pos_neg().1));
    for d in 1..8_u32 {
        for nw in 0..2_u32 {
            u(&format!("trunc{d}_{nw}"), Box::new(move |a| if d < a.width { a.truncate(d, nw) } else { *a }));
        }
    }
    for c in 0..4_u32 {
        u(
            &format!("castF2I{c}"),
            Box::new(move |a| {
                if a.width + 2 > 64 {
                    return *a;
                }
                let (op, width) = match c {
                    0 => (CastOp::UIToFP, a.width + 2),
                    1 => (CastOp::SIToFP, a.width + 2),
                    2 => (CastOp::FPToUI, a.width),
                    _ => (CastOp::BitCast, a.width),
                };
                a.cast_op(op, width)
            }),
        );
    }
    v
}

fn golden_lines() -> Vec<String> {
    let mut h = Hash(0xcbf29ce484222325);
    let mut lines = Vec::new();
    let (bins, uns) = (binaries(), unaries());
    for width in [1_u32, 2, 3, 4, 6, 8, 16, 64] {
        let input = ranges(width);
        let m = mask64(width);
        for (name, f) in &bins {
            for a in &input {
                for b in &input {
                    h.out(&f(a, b));
                }
            }
            h.line(&mut lines, name, width);
        }
        for (name, f) in &uns {
            for a in &input {
                h.out(&f(a));
            }
            h.line(&mut lines, name, width);
        }
        for p in PREDICATES {
            for a in &input {
                for b in &input {
                    h.mix(u64::from(a.icmp(p, b)));
                }
            }
            h.line(&mut lines, &format!("icmp{}", predicate_number(p)), width);
        }
        for a in &input {
            for b in &input {
                h.mix(u64::from(a.contains_range(b)));
                h.mix(u64::from(a.is_size_strictly_smaller_than(b)));
                h.mix(u64::from(a == b));
                h.mix(a.unsigned_add_may_overflow(b) as u64);
                h.mix(a.signed_add_may_overflow(b) as u64);
                h.mix(a.unsigned_sub_may_overflow(b) as u64);
                h.mix(a.signed_sub_may_overflow(b) as u64);
                h.mix(a.unsigned_mul_may_overflow(b) as u64);
                h.mix(u64::from(CR::are_insensitive_to_signedness_of_icmp_predicate(a, b)));
                h.mix(u64::from(CR::are_insensitive_to_signedness_of_inverted_icmp_predicate(a, b)));
                let ei = a.exact_intersect_with(b);
                h.mix(u64::from(ei.is_some()));
                if let Some(r) = ei {
                    h.out(&r);
                }
                let eu = a.exact_union_with(b);
                h.mix(u64::from(eu.is_some()));
                if let Some(r) = eu {
                    h.out(&r);
                }
                for p in &PREDICATES[2..] {
                    h.mix(CR::get_equivalent_pred_with_flipped_signedness(*p, a, b).map_or(42, predicate_number));
                }
            }
        }
        h.line(&mut lines, "pairQueries", width);
        for a in &input {
            for v in [a.get_unsigned_min(), a.get_unsigned_max(), a.get_signed_min(), a.get_signed_max()] {
                h.mix(v as u64);
            }
            for v in [
                a.is_full_set(),
                a.is_empty_set(),
                a.is_wrapped_set(),
                a.is_upper_wrapped(),
                a.is_sign_wrapped_set(),
                a.is_upper_sign_wrapped(),
                a.is_all_negative(),
                a.is_all_non_negative(),
                a.is_all_positive(),
            ] {
                h.mix(u64::from(v));
            }
            h.mix(u64::from(a.get_active_bits()));
            h.mix(u64::from(a.get_min_signed_bits()));
            h.mix(u64::from(a.is_single_element()));
            h.mix(a.get_single_element().map_or(99, |v| v as u64));
            h.mix(a.get_single_missing_element().map_or(99, |v| v as u64));
            for k in [1_u64, 2, 3, 4, 100, 0] {
                h.mix(u64::from(a.is_size_larger_than(k)));
            }
            let (p, rhs, offset) = a.get_equivalent_icmp_offset();
            h.mix(predicate_number(p));
            h.mix(rhs as u64);
            h.mix(offset as u64);
            h.out_known(&a.to_known_bits());
        }
        h.line(&mut lines, "queries", width);
        if width <= 8 {
            for a in &input {
                for v in 0..=m {
                    if v >= 64 {
                        break;
                    }
                    h.mix(u64::from(a.contains(u128::from(v))));
                    h.out(&a.subtract(u128::from(v)));
                }
            }
            h.line(&mut lines, "containsSubtract", width);
        }
        for v in 0..=m {
            if v >= 16 {
                break;
            }
            let v = u128::from(v);
            for p in PREDICATES {
                h.out(&CR::make_exact_icmp_region(p, width, v));
            }
            for k in [1_u32, 2] {
                for op in [BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul, BinaryOp::Shl] {
                    h.out(&CR::make_exact_no_wrap_region(op, width, v, k));
                }
            }
        }
        h.line(&mut lines, "exactRegions", width);
        if width <= 4 {
            for mask_bits in 0..=m {
                for c in 0..=m {
                    h.out(&CR::make_mask_not_equal_range(width, u128::from(mask_bits), u128::from(c)));
                }
            }
            h.line(&mut lines, "maskNotEqual", width);
            for c in 0..3_u64.pow(width) {
                let (mut zero, mut one, mut t) = (0_u128, 0_u128, c);
                for i in 0..width {
                    match t % 3 {
                        1 => zero |= 1 << i,
                        2 => one |= 1 << i,
                        _ => {}
                    }
                    t /= 3;
                }
                let k = KnownBits::from_masks(width, zero, one);
                h.out(&CR::from_known_bits(&k, false));
                h.out(&CR::from_known_bits(&k, true));
            }
            h.line(&mut lines, "fromKnownBits", width);
        }
    }
    lines
}

/// Each operation's answers agree with LLVM's over every range: a line that
/// differs names the operation and the width.
#[test]
fn test_every_operation_agrees_with_llvm() {
    let golden: Vec<&str> = include_str!("constantrange_golden.txt").lines().collect();
    let ours = golden_lines();
    let differing: Vec<String> =
        ours.iter().zip(&golden).filter(|(a, b)| a.as_str() != **b).map(|(a, b)| format!("{a} (LLVM: {b})")).collect();
    assert_eq!(ours.len(), golden.len(), "{} operations here, {} in LLVM's file", ours.len(), golden.len());
    assert!(differing.is_empty(), "{} differ:\n{}", differing.len(), differing.join("\n"));
}

/// A range prints as LLVM prints it.
#[test]
fn test_print_names_the_full_and_empty_sets() {
    assert_eq!(CR::full(8).print(), "full-set");
    assert_eq!(CR::empty(8).print(), "empty-set");
    assert_eq!(CR::new(8, 250, 5).print(), "[-6,5)", "bounds print signed, as LLVM's APInt does");
}
