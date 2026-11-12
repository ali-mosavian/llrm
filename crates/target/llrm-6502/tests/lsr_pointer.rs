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

/// rep_count's `for (i...) for (j...) s += g[i][j]` over rows of 54 bytes.
/// The pin's LSR skips the outer loop, so `i * 54` fed the inner loop through
/// `__mulhi3` on every row: rep_count -O1 kept the call (2026-11-11).
const NEST: &str = "@g = global [16 x [54 x i8]] zeroinitializer\n\
    define i32 @f() {\nentry:\n  br label %outer\n\
    outer:\n  %s = phi i32 [0, %entry], [%s1, %latch]\n  %i = phi i16 [0, %entry], [%i1, %latch]\n  \
    %row = getelementptr inbounds nuw [54 x i8], ptr @g, i16 %i\n  br label %inner\n\
    inner:\n  %t = phi i32 [%s, %outer], [%t1, %inner]\n  %j = phi i16 [0, %outer], [%j1, %inner]\n  \
    %p = getelementptr inbounds nuw i8, ptr %row, i16 %j\n  %v = load i8, ptr %p\n  %x = sext i8 %v to i32\n  \
    %t1 = add nsw i32 %t, %x\n  %j1 = add nuw nsw i16 %j, 1\n  %jc = icmp samesign ult i16 %j, 53\n  \
    br i1 %jc, label %inner, label %latch\n\
    latch:\n  %s1 = phi i32 [%t1, %inner]\n  %i1 = add nuw nsw i16 %i, 1\n  %ic = icmp samesign ult i16 %i, 15\n  \
    br i1 %ic, label %outer, label %out\n\
    out:\n  ret i32 %s1\n}\n";

#[test]
fn an_outer_loops_row_address_steps_by_the_row_size_and_calls_no_multiply() {
    let ir = format!("target datalayout = \"{}\"\n{NEST}", datalayout());
    for level in [Level::O1, Level::O2] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        let body = text.split(".segment").next().unwrap();
        assert!(!body.contains("mulhi"), "{level:?}: {body}");
    }
}


/// word_compares at -O2: with a narrowed `i8` counter the loop kept
/// `gep v, zext(j)` and paid `zext; asl; rol; adc` per trip where a pointer
/// phi steps by 4 (LU19 kept the extended index out of LSR whatever the element size, 2026-11-12).
const NARROW_INDEX: &str = "@v = global [16 x i32] zeroinitializer\n\
    define i32 @f() {\nentry:\n  br label %l\n\
    l:\n  %s = phi i32 [0, %entry], [%s1, %l]\n  %j = phi i8 [0, %entry], [%j1, %l]\n  \
    %w = zext i8 %j to i16\n  %p = getelementptr inbounds nuw [4 x i8], ptr @v, i16 %w\n  \
    %x = load volatile i32, ptr %p\n  %s1 = add i32 %s, %x\n  %j1 = add nuw nsw i8 %j, 1\n  \
    %c = icmp ult i8 %j1, 14\n  br i1 %c, label %l, label %out\n\
    out:\n  ret i32 %s1\n}\n";

#[test]
fn an_address_rebuilt_from_a_narrow_counter_becomes_a_pointer() {
    let ir = format!("target datalayout = \"{}\"\n{NARROW_INDEX}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    assert!(!body.contains("asl"), "{body}");
}

/// ring at -O2: `buf[i] = (i + 1) * 3 + n` over a byte counter kept
/// `asl; adc` of the extended counter each trip, where the 16-bit counter
/// stepped `adc #3` (frameless -O2 +2,611 clk, 2026-11-12). A constant
/// multiply scales the extension like an address does.
const SCALED_COUNTER: &str = "define i32 @f(i32 %n) {\nentry:\n  br label %l\n\
    l:\n  %j = phi i8 [0, %entry], [%j1, %l]\n  %j1 = add nuw nsw i8 %j, 1\n  \
    %w = zext i8 %j1 to i16\n  %m = mul nuw nsw i16 %w, 3\n  %e = zext i16 %m to i32\n  \
    %x = add i32 %n, %e\n  store volatile i32 %x, ptr @g\n  \
    %c = icmp ult i8 %j1, 64\n  br i1 %c, label %l, label %out\n\
    out:\n  ret i32 0\n}\n@g = global i32 0\n";

#[test]
fn a_constant_multiple_of_a_narrow_counter_steps_instead_of_being_rebuilt() {
    let ir = format!("target datalayout = \"{}\"\n{SCALED_COUNTER}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    assert!(!body.contains("asl"), "{body}");
}
