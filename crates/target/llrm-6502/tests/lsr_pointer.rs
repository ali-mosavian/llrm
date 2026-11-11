//! LSR dropped every 16-bit pointer and index on this target (`n8`: no 16-bit
//! integer is legal), so `g[i]` for a 54-byte row called `__mulhi3` on each
//! trip: rep_count at -O1 spent 148,120 clocks where 133,418 do (2026-11-10).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `for (i = 0; i < n; i++) g[i][0] = 1;` over rows of 54 bytes.
const ROWS: &str = "@g = global [16 x [54 x i8]] zeroinitializer\n\
    define void @f(i16 %n) {\nentry:\n  %z = icmp eq i16 %n, 0\n  br i1 %z, label %out, label %l\n\
    l:\n  %i = phi i16 [0, %entry], [%i1, %l]\n  %p = getelementptr [54 x i8], ptr @g, i16 %i\n  \
    store i8 1, ptr %p\n  %i1 = add i16 %i, 1\n  %c = icmp ne i16 %i1, %n\n  br i1 %c, label %l, label %out\n\
    out:\n  ret void\n}\n";

#[test]
fn a_row_address_steps_by_the_row_size_and_calls_no_multiply() {
    let ir = format!("target datalayout = \"{}\"\n{ROWS}", datalayout());
    // -Os keeps the pin's widths (DEVIATE LSR-N holds at speed only).
    for level in [Level::O1, Level::O2] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        let body = text.split(".segment").next().unwrap();
        assert!(!body.contains("mulhi"), "{level:?}: {body}");
    }
}

fn cost(
    insns: u32,
    num_regs: u32,
    add_rec_cost: u32,
    num_base_adds: u32,
    setup_cost: u32,
) -> llrm_mir::tti::LsrCost {
    llrm_mir::tti::LsrCost {
        insns,
        num_regs,
        add_rec_cost,
        num_iv_muls: 0,
        num_base_adds,
        imm_cost: 0,
        setup_cost,
        scale_cost: 0,
        spills: 0,
    }
}

/// switch_empty_cases at -O1 grew 106 B to 153 B and 883 to 990 clocks: LSR
/// took the solution with fewer registers (2 against 3) that added four 16-bit
/// base adds, because the pin's order ranks registers first. Zero-page cells
/// are cheap; a 16-bit add is seven instructions.
#[test]
fn a_solution_with_more_work_loses_to_one_with_more_cells() {
    let tti = llrm_6502::driver::machine().tti().expect("a cost model");
    let baseline = cost(3, 3, 2, 1, 4);
    let reduced = cost(5, 2, 1, 4, 3);
    assert!(tti.is_lsr_cost_less(&baseline, &reduced), "baseline {baseline:?} is cheaper than {reduced:?}");
    assert!(!tti.is_lsr_cost_less(&reduced, &baseline));
}

/// Without a legal add or compare immediate LSR bakes each offset into a
/// counter of its own (switch_jump_table +177 B, +24% clocks); `adc #k` and
/// `cmp #k` take any constant, a byte at a time.
#[test]
fn an_immediate_add_or_compare_is_legal() {
    let tti = llrm_6502::driver::machine().tti().expect("a cost model");
    for imm in [1, -1, 255, 256, -300, 70000] {
        assert!(tti.is_legal_add_immediate(imm) && tti.is_legal_icmp_immediate(imm), "{imm}");
    }
}

/// A rejected formula carries `u32::MAX` terms. The weighted sum overflowed
/// `u32`: a debug build panicked, a release build wrapped and could rank the
/// rejected formula cheapest.
#[test]
fn a_rejected_formula_loses_without_overflow() {
    let tti = llrm_6502::driver::machine().tti().expect("a cost model");
    let loser = llrm_mir::tti::LsrCost {
        insns: u32::MAX,
        num_regs: u32::MAX,
        add_rec_cost: u32::MAX,
        num_iv_muls: u32::MAX,
        num_base_adds: u32::MAX,
        imm_cost: u32::MAX,
        setup_cost: u32::MAX,
        scale_cost: u32::MAX,
        spills: 0,
    };
    let ok = cost(3, 3, 2, 1, 4);
    assert!(tti.is_lsr_cost_less(&ok, &loser));
    assert!(!tti.is_lsr_cost_less(&loser, &ok));
}

/// `do i++; while (i != n)` whose bound is read after the loop too. The bound
/// is a cell the loop and the code after it share, so the count-down from it
/// costs no cell the loop as written does not: the baseline left that cell
/// out, the solution paid for it, and LSR dropped the count-down on the tie
/// (known_loads_walks -O1 kept `cpx rc2`, +1,344 clocks, 2026-11-11).
const SPIN: &str = "@g = global i8 0\n\
    define i8 @f(i8 %n) {\nentry:\n  br label %l\n\
    l:\n  %i = phi i8 [0, %entry], [%i1, %l]\n  store volatile i8 3, ptr @g\n  %i1 = add nuw i8 %i, 1\n  \
    %c = icmp eq i8 %i1, %n\n  br i1 %c, label %out, label %l\n\
    out:\n  store volatile i8 %n, ptr @g\n  ret i8 0\n}\n";

#[test]
fn a_count_down_to_a_bound_the_code_after_the_loop_reads_is_kept() {
    let ir = format!("target datalayout = \"{}\"\n{SPIN}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O1)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    assert!(!body.contains("cpx rc") && !body.contains("cmp rc"), "{body}");
}
