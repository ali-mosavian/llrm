//! Port of `qbopt/cycles/timings.py`.
//!
//! Vendored from the runtime pass (`~/work/badlogic/mgl, tools/cycles/timings.py`,
//! taken 2026-08-29). Legacy approximate rankings, partly recalled from timing
//! tables; not uniformly verified latencies or reciprocal throughputs. Audited
//! form-specific bounds live in backend/timing.py; see docs/measurement/timing-audit.md.
//!
//! Reciprocal throughput in cycles where the distinction matters. ALU and mov
//! entries are solid, divide entries the roughest, K5 the least certain column.
//! Where a cost depends on the operand (idiv) the figure is for a 32 bit
//! register operand. Correct them in place.
//!
//! Python builds `COST` and `LATENCY` by mutating both in one module body;
//! `_MODULE` runs that body in the same order.

use std::sync::LazyLock;

use crate::support::hash::IndexMap;

pub const ARCHS: [&str; 7] = ["486", "P5", "P6", "K5", "K6", "K7", "Core"];

/// A 16 bit write followed by a 32 bit read of the same register stalls the P6
/// while it merges the halves; Core recovers most of it with a merging uop.
//                                   486 P5 P6 K5 K6 K7 Core
pub const PARTIAL_STALL: [i64; 7] = [0, 0, 7, 0, 1, 1, 2];

/// A 66h prefix on an instruction with an immediate changes its length and
/// stalls the decoder on P6 and Core.
//                               486 P5 P6 K5 K6 K7 Core
pub const LCP_STALL: [i64; 7] = [0, 0, 6, 0, 0, 0, 3];

/// Instructions retired per cycle.
//                           486 P5 P6 K5 K6 K7 Core
pub const ISSUE: [i64; 7] = [1, 2, 3, 4, 3, 3, 4];

/// Only the first two are in order.
//                             486 P5 P6 K5 K6 K7 Core
pub const INORDER: [i64; 7] = [1, 1, 0, 0, 0, 0, 0];

/// The 486 and P5 spend about a clock decoding each prefix, and every widened
/// instruction carries a 66h in 16-bit code.
//                            486 P5 P6 K5 K6 K7 Core
pub const PREFIX: [i64; 7] = [1, 1, 0, 0, 0, 0, 0];

// x87 forms from the C frontend's lowering: target-ranking units. K5 has no
// published FDIV timing, so its division entry keeps the K6 ranking.
//                                486 P5 P6 K5 K6 K7 Core
const _X87_LOAD: [i64; 7] = [8, 2, 2, 6, 6, 4, 6];
// FXCH: 486 four-clock form; P5 pairs; P6/Core add no latency.
const _X87_EXCHANGE: [i64; 7] = [4, 1, 0, 2, 2, 2, 0];
const _X87_STORE: [i64; 7] = [8, 4, 4, 6, 4, 6, 6];
const _X87_ADD: [i64; 7] = [8, 3, 3, 5, 2, 4, 3];
const _X87_MUL: [i64; 7] = [16, 3, 5, 8, 2, 4, 5];
const _X87_DIV: [i64; 7] = [73, 39, 56, 56, 56, 24, 24];

// Memory arithmetic: K5 published directly; the rest compose arithmetic + load.
const _X87_ADD_M: [i64; 7] = [16, 5, 5, 7, 8, 8, 9];
const _X87_MUL_M: [i64; 7] = [24, 5, 7, 10, 8, 8, 11];
const _X87_DIV_M: [i64; 7] = [81, 41, 58, 62, 62, 28, 30];

/// What an instruction kind occupies, per arch.
pub static COST: LazyLock<IndexMap<&'static str, [i64; 7]>> = LazyLock::new(|| _MODULE.0.clone());

/// What an instruction kind delays its dependents, per arch. A sequence scores
/// the larger of what it occupies and what it depends on.
pub static LATENCY: LazyLock<IndexMap<&'static str, [i64; 7]>> =
    LazyLock::new(|| _MODULE.1.clone());

/// The Python module body, which interleaves mutations of `COST` and `LATENCY`.
#[allow(clippy::type_complexity)]
static _MODULE: LazyLock<(
    IndexMap<&'static str, [i64; 7]>,
    IndexMap<&'static str, [i64; 7]>,
)> = LazyLock::new(|| {
    //                         486  P5  P6  K5  K6  K7 Core
    let mut cost: IndexMap<&'static str, [i64; 7]> = IndexMap::from_iter([
        ("alu_rr", [1, 1, 1, 1, 1, 1, 1]), // add/and/or/xor/sub/cmp reg,reg
        ("alu_rm", [2, 2, 1, 1, 1, 1, 1]), // ... reg,[mem]
        ("alu_mr", [3, 3, 1, 1, 1, 1, 1]), // ... [mem],reg
        ("mov_rr", [1, 1, 1, 1, 1, 1, 1]),
        ("mov_rm", [1, 1, 1, 1, 1, 1, 1]),
        ("mov_mr", [1, 1, 1, 1, 1, 1, 1]),
        ("mov_ri", [1, 1, 1, 1, 1, 1, 1]),
        ("shift_ri", [2, 1, 1, 1, 1, 1, 1]), // shl/shr/sar reg,imm
        ("shift_r1", [3, 1, 1, 1, 1, 1, 1]), // the D1 form, reg,1
        ("movzx", [3, 3, 1, 1, 1, 1, 1]),
        ("cdq", [3, 2, 1, 1, 1, 1, 1]),
        ("imul_r32", [26, 10, 4, 4, 3, 5, 3]), // 486 is 13-42, data dependent
        ("imul_m32", [27, 11, 4, 4, 3, 5, 3]),
        ("idiv_r32", [43, 46, 39, 42, 41, 40, 26]), // the expensive one
        ("idiv_m32", [44, 47, 39, 42, 41, 40, 26]),
        ("push_r", [1, 1, 1, 1, 1, 1, 1]),
        ("push_m", [4, 2, 2, 2, 2, 2, 2]),
        ("push_i", [1, 1, 1, 1, 1, 1, 1]),
        ("pop_r", [4, 1, 1, 1, 1, 1, 1]),
        // POP memory is an implicit stack load and an explicit store. 486
        // uses Intel's six-clock form; P5/P6/K6/K7/Core follow GCC's
        // scheduling descriptions; K5 has none and uses K6's form.
        ("pop_m", [6, 1, 4, 3, 3, 4, 4]),
        ("pop_seg", [3, 3, 8, 3, 3, 5, 8]), // segment load, costly on P6+
        ("mov_seg_r", [3, 3, 8, 3, 3, 5, 8]),
        ("les", [6, 4, 9, 4, 4, 6, 9]),
        ("nop", [1, 1, 1, 1, 1, 1, 1]),
        ("jmp_short", [3, 1, 1, 1, 1, 1, 1]), // predicted taken
        ("jcc", [3, 1, 1, 1, 1, 1, 1]),       // predicted
        ("call_far", [18, 4, 21, 4, 4, 5, 22]), // real mode, no gate
        ("ret_far", [13, 4, 17, 4, 4, 5, 18]),
        ("unknown", [2, 2, 2, 2, 2, 2, 2]),
    ]);

    //                            486  P5  P6  K5  K6  K7 Core
    let mut latency: IndexMap<&'static str, [i64; 7]> = IndexMap::from_iter([
        ("alu_rr", [1, 1, 1, 1, 1, 1, 1]),
        ("alu_rm", [2, 2, 4, 3, 3, 3, 4]), // + load
        ("alu_mr", [3, 3, 4, 3, 3, 3, 4]),
        ("mov_rr", [1, 1, 1, 1, 1, 1, 1]),
        ("mov_rm", [1, 1, 3, 2, 2, 3, 4]),
        ("mov_mr", [1, 1, 3, 2, 2, 3, 3]),
        ("mov_ri", [1, 1, 1, 1, 1, 1, 1]),
        ("shift_ri", [2, 1, 1, 1, 1, 1, 1]),
        ("shift_r1", [3, 1, 1, 1, 1, 1, 1]),
        ("movzx", [3, 3, 1, 1, 1, 1, 1]),
        ("cdq", [3, 2, 1, 1, 1, 1, 1]),
        ("imul_r32", [26, 10, 4, 4, 3, 5, 3]),
        ("imul_m32", [27, 11, 7, 6, 5, 7, 6]),
        ("mul_r16", [13, 11, 4, 4, 3, 5, 3]),
        ("idiv_r32", [43, 46, 39, 42, 41, 40, 26]),
        ("idiv_m32", [44, 47, 39, 42, 41, 40, 26]),
        ("div_r16", [24, 25, 23, 24, 24, 24, 22]),
        ("push_r", [1, 1, 1, 1, 1, 1, 1]),
        ("push_m", [4, 2, 3, 2, 2, 2, 3]),
        ("push_i", [1, 1, 1, 1, 1, 1, 1]),
        ("pop_r", [4, 1, 3, 2, 2, 2, 3]),
        ("pop_m", [6, 1, 4, 3, 3, 4, 4]),
        ("pop_seg", [3, 3, 8, 3, 3, 5, 8]),
        ("mov_seg_r", [3, 3, 8, 3, 3, 5, 8]),
        ("les", [6, 4, 9, 4, 4, 6, 9]),
        ("nop", [1, 1, 1, 1, 1, 1, 1]),
        ("jmp_short", [3, 1, 1, 1, 1, 1, 1]),
        ("jcc", [3, 1, 1, 1, 1, 1, 1]),
        ("call_far", [18, 4, 21, 4, 4, 5, 22]),
        ("ret_far", [13, 4, 17, 4, 4, 5, 18]),
        ("lahf", [3, 2, 3, 2, 2, 2, 3]),
        ("sahf", [2, 2, 1, 1, 1, 1, 1]),
        ("unknown", [2, 2, 2, 2, 2, 2, 2]),
    ]);
    for (k, v) in &cost {
        latency.entry(*k).or_insert(*v);
    }
    cost.entry("mul_r16").or_insert([13, 11, 4, 4, 3, 5, 3]);
    cost.entry("div_r16")
        .or_insert([24, 25, 23, 24, 24, 24, 22]);
    cost.entry("lahf").or_insert([3, 2, 3, 2, 2, 2, 3]);
    cost.entry("sahf").or_insert([2, 2, 1, 1, 1, 1, 1]);

    // lea does a shift and an add without touching flags; in 16-bit code
    // the scaled forms need a 67h prefix.
    //                              486  P5  P6  K5  K6  K7 Core
    cost.insert("lea", [2, 1, 1, 1, 1, 1, 1]);
    latency.insert("lea", [2, 1, 1, 1, 1, 1, 1]);

    // REP STOS: a setup, then a clock count per cell. The 386 and 486 figures
    // are Intel's (5+5n, 7+4n); the rest are Agner Fog's small-count rankings,
    // where fast strings have not yet paid for their startup.
    //                              486  P5  P6  K5  K6  K7 Core
    cost.insert("rep_stos", [7, 9, 30, 10, 10, 15, 30]);
    cost.insert("rep_stos_cell", [4, 1, 1, 1, 1, 1, 1]);
    latency.insert("rep_stos", cost["rep_stos"]);
    latency.insert("rep_stos_cell", cost["rep_stos_cell"]);

    // 32-bit multiply, for telling it from the 16-bit one
    cost.insert("mul_r32", [26, 10, 4, 4, 3, 5, 3]);
    latency.insert("mul_r32", [26, 10, 4, 4, 3, 5, 3]);

    cost.extend([
        // LEAVE is the frame-register move plus POP ranking.
        ("leave", [5, 2, 2, 2, 2, 2, 2]),
        ("x87_load", _X87_LOAD),
        ("x87_exchange", _X87_EXCHANGE),
        ("x87_store", _X87_STORE),
        // Conversion plus store, except K5 whose FISTP int64 is published
        // directly as seven cycles.
        ("x87_convert_store", [35, 7, 7, 7, 6, 12, 12]),
        ("x87_add", _X87_ADD),
        ("x87_add_m", _X87_ADD_M),
        ("x87_mul", _X87_MUL),
        ("x87_mul_m", _X87_MUL_M),
        ("x87_div", _X87_DIV),
        ("x87_div_m", _X87_DIV_M),
        // Control-word transfers rank as memory transfers.
        ("x87_control_load", _X87_LOAD),
        ("x87_control_store", _X87_STORE),
    ]);

    // No second number is established for these forms, so matching
    // values are explicit instead of falling through to `unknown`.
    for _form in [
        "leave",
        "x87_load",
        "x87_exchange",
        "x87_store",
        "x87_convert_store",
        "x87_add",
        "x87_add_m",
        "x87_mul",
        "x87_mul_m",
        "x87_div",
        "x87_div_m",
        "x87_control_load",
        "x87_control_store",
    ] {
        latency.insert(_form, cost[_form]);
    }
    (cost, latency)
});

#[cfg(test)]
mod tests {
    use super::*;

    /// Key order and rows printed by
    /// `uv run python -c "from qbopt.cycles import timings as t; print(list(t.COST)); ..."`
    /// on 2026-09-22. Order matters: callers iterate these tables.
    #[test]
    fn test_tables_match_python() {
        let cost_keys: Vec<&str> = COST.keys().copied().collect();
        assert_eq!(
            cost_keys,
            [
                "alu_rr",
                "alu_rm",
                "alu_mr",
                "mov_rr",
                "mov_rm",
                "mov_mr",
                "mov_ri",
                "shift_ri",
                "shift_r1",
                "movzx",
                "cdq",
                "imul_r32",
                "imul_m32",
                "idiv_r32",
                "idiv_m32",
                "push_r",
                "push_m",
                "push_i",
                "pop_r",
                "pop_m",
                "pop_seg",
                "mov_seg_r",
                "les",
                "nop",
                "jmp_short",
                "jcc",
                "call_far",
                "ret_far",
                "unknown",
                "mul_r16",
                "div_r16",
                "lahf",
                "sahf",
                "lea",
                "rep_stos",
                "rep_stos_cell",
                "mul_r32",
                "leave",
                "x87_load",
                "x87_exchange",
                "x87_store",
                "x87_convert_store",
                "x87_add",
                "x87_add_m",
                "x87_mul",
                "x87_mul_m",
                "x87_div",
                "x87_div_m",
                "x87_control_load",
                "x87_control_store",
            ]
        );
        let latency_keys: Vec<&str> = LATENCY.keys().copied().collect();
        assert_eq!(
            latency_keys,
            [
                "alu_rr",
                "alu_rm",
                "alu_mr",
                "mov_rr",
                "mov_rm",
                "mov_mr",
                "mov_ri",
                "shift_ri",
                "shift_r1",
                "movzx",
                "cdq",
                "imul_r32",
                "imul_m32",
                "mul_r16",
                "idiv_r32",
                "idiv_m32",
                "div_r16",
                "push_r",
                "push_m",
                "push_i",
                "pop_r",
                "pop_m",
                "pop_seg",
                "mov_seg_r",
                "les",
                "nop",
                "jmp_short",
                "jcc",
                "call_far",
                "ret_far",
                "lahf",
                "sahf",
                "unknown",
                "lea",
                "rep_stos",
                "rep_stos_cell",
                "mul_r32",
                "leave",
                "x87_load",
                "x87_exchange",
                "x87_store",
                "x87_convert_store",
                "x87_add",
                "x87_add_m",
                "x87_mul",
                "x87_mul_m",
                "x87_div",
                "x87_div_m",
                "x87_control_load",
                "x87_control_store",
            ]
        );
        for (k, row) in [
            ("pop_m", [6, 1, 4, 3, 3, 4, 4]),
            ("mul_r16", [13, 11, 4, 4, 3, 5, 3]),
            ("lea", [2, 1, 1, 1, 1, 1, 1]),
            ("x87_div_m", [81, 41, 58, 62, 62, 28, 30]),
            ("x87_control_store", [8, 4, 4, 6, 4, 6, 6]),
        ] {
            assert_eq!(COST[k], row, "COST[{k}]");
            assert_eq!(LATENCY[k], row, "LATENCY[{k}]");
        }
        assert_eq!(LATENCY["alu_rm"], [2, 2, 4, 3, 3, 3, 4]);
        assert_eq!(COST["alu_rm"], [2, 2, 1, 1, 1, 1, 1]);
    }
}
