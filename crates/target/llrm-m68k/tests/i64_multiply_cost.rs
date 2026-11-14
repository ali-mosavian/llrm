//! The pin's M68k prices an `i64` multiply 1 under the cost kinds the loop
//! unroller asks (`opt -mtriple=m68k-unknown-linux-gnu -cost-kind=size-latency
//! -passes='print<cost-model>'`: a variable and 3, 9, 100, 1431655765: 1, at
//! both CPUs). Priced as a call (4) for every constant, the multiplies by 2, 3
//! and 5 in `compare_to_int64`'s 3x3 loops made the inner loop too big to
//! unroll under the m68k's threshold of 32: 142 instructions stayed at -O3
//! (572 B at the 68000, gcc's 30). A multiply the lowering does call keeps the
//! price of a call (DEVIATE, `arithmetic_cost`); the price follows the
//! lowering, constant by constant, in a loop body: where the unroller asks, and
//! where a chain's clocks count (outside a loop the byte trial keeps the call
//! for -38 and -26). Since #9197 the lowering takes a chain for every word
//! multiply the search prices, and the 68000 price still called for ~100 such
//! constants (mul i64 by 100: price 4, lowering shifts and adds).

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};
use llrm_m68k::machine::M68k;
use llrm_mir::tti::{ArithOp, Cost, CostKind, OperandInfo, Tti, Ty};

const HEAD: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

fn mul64(
    cpu: Cpu,
    constant: Option<i128>,
) -> Cost {
    let right = constant.map_or(OperandInfo::ANY, |value| OperandInfo::constant(value, 64));
    M68k::for_cpu(cpu).arithmetic_cost(ArithOp::Mul, Ty::Int(64), CostKind::SizeAndLatency, OperandInfo::ANY, right)
}

fn lowers_to_a_call(
    cpu: Cpu,
    constant: Option<i64>,
) -> bool {
    let right = constant.map_or("%b".to_string(), |value| value.to_string());
    let ir = format!(
        "{HEAD}define i64 @f(i64 %a, i64 %b, i32 %n) {{\nentry:\n  br label %loop\nloop:\n  \
         %i = phi i32 [0, %entry], [%i1, %loop]\n  %acc = phi i64 [%a, %entry], [%r, %loop]\n  \
         %r = mul i64 %acc, {right}\n  %i1 = add i32 %i, 1\n  %c = icmp ult i32 %i1, %n\n  \
         br i1 %c, label %loop, label %exit\nexit:\n  ret i64 %r\n}}\n"
    );
    let text = assembly(&ir, cpu, Level::O2).expect("compiles");
    text.contains("__muldi3") || text.contains("__mulsi3")
}

#[test]
fn the_price_of_an_i64_multiply_follows_the_lowering() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let constants = [-38, -34, -26, -3, 2, 3, 13, 38, 100, 125, 130, 1431655765, 100_000, -100_000, 1 << 40]
            .into_iter()
            .map(Some)
            .chain([None]);
        for constant in constants {
            let expected = if lowers_to_a_call(cpu, constant) { Cost::EXPENSIVE } else { Cost::of(1) };
            assert_eq!(mul64(cpu, constant.map(i128::from)), expected, "{cpu:?} mul i64 by {constant:?}");
        }
    }
}

#[test]
fn a_68020_multiply_by_any_constant_costs_one() {
    for constant in [Some(3), Some(100), Some(-3), Some(1431655765), None] {
        assert_eq!(mul64(Cpu::M68020, constant), Cost::of(1), "mul i64 by {constant:?}");
    }
}

#[test]
fn a_68000_multiply_by_a_constant_is_a_chain_and_by_a_variable_a_call() {
    for constant in [Some(2), Some(3), Some(5), Some(9), Some(15), Some(-8), Some(-3), Some(100), Some(1 << 40), Some(100_000), Some(1431655765)] {
        assert_eq!(mul64(Cpu::M68000, constant), Cost::of(1), "mul i64 by {constant:?}");
    }
    for constant in [None] {
        assert_eq!(mul64(Cpu::M68000, constant), Cost::EXPENSIVE, "mul i64 by {constant:?}");
    }
}
