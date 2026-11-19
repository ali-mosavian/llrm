//! DEVIATE BFS-1: bounds checks (`!prof {1048575, 1}` to a `noreturn` sink) in two successive loop nests leak so much mass that the
//! call after them reads 0.0011 of the entry, cold, and the inliner held it to threshold 45 (Nib `jpeg_single` -O1 m16: `convert` and
//! `restart` stayed calls, +62 B). m16 prices the cold-site test on a flow without the failure edges.

use std::rc::Rc;

use llrm_mir::program::Program;
use llrm_mir::target::Machine;
use llrm_transforms::pipeline::{Applied, Options, applied};
use llrm_x86_m16::target::Dos;

/// Two nests of six loops, a bounds check in each innermost (or none) and a volatile store so the loops are kept, then a call of a 40-operation local function.
fn source(checked: bool) -> String {
    let (depth, nests) = (6, 2);
    let mut text = String::from("@sink = global i16 0\ndeclare void @trap() noreturn\n\ndefine internal i16 @leaf(i16 %x) {\nentry:\n  %a0 = mul i16 %x, %x\n");
    for k in 1..40 {
        let op = if k % 2 == 0 { "mul" } else { "xor" };
        text += &format!("  %a{k} = {op} i16 %a{}, %x\n", k - 1);
    }
    text += "  ret i16 %a39\n}\n\ndefine i16 @main(i16 %n, i16 %x) {\nentry:\n  br label %a0.h0\n\n";
    for a in 0..nests {
        for k in 0..depth {
            let last = k == depth - 1;
            let pre = match (a, k) {
                (0, 0) => "entry".to_string(),
                (_, 0) => format!("a{}.out", a - 1),
                _ => format!("a{a}.p{k}"),
            };
            let body = if last { format!("a{a}.chk") } else { format!("a{a}.p{}", k + 1) };
            text += &format!(
                "a{a}.h{k}:\n  %a{a}.i{k} = phi i16 [ 0, %{pre} ], [ %a{a}.j{k}, %a{a}.l{k} ]\n  %a{a}.c{k} = icmp slt i16 %a{a}.i{k}, %n\n  br i1 %a{a}.c{k}, label %{body}, label %a{a}.x{k}\n\n"
            );
            if !last {
                text += &format!("a{a}.p{}:\n  br label %a{a}.h{}\n\n", k + 1, k + 1);
            }
            let next = if k == 0 { format!("a{a}.out") } else { format!("a{a}.l{}", k - 1) };
            text += &format!(
                "a{a}.x{k}:\n  br label %{next}\n\na{a}.l{k}:\n  %a{a}.j{k} = add i16 %a{a}.i{k}, 1\n  br label %a{a}.h{k}\n\n"
            );
        }
        let onward = if a + 1 < nests { format!("a{}.h0", a + 1) } else { "call".to_string() };
        let inner = depth - 1;
        text += &if checked {
            format!(
                "a{a}.chk:\n  store volatile i16 %a{a}.i{inner}, ptr @sink\n  %a{a}.ok = icmp ult i16 %a{a}.i{inner}, 64\n  br i1 %a{a}.ok, label %a{a}.l{inner}, label %a{a}.fail, !prof !0\n\n\
                 a{a}.fail:\n  call void @trap()\n  unreachable\n\na{a}.out:\n  br label %{onward}\n\n"
            )
        } else {
            format!("a{a}.chk:\n  store volatile i16 %a{a}.i{inner}, ptr @sink\n  br label %a{a}.l{inner}\n\na{a}.out:\n  br label %{onward}\n\n")
        };
    }
    text += "call:\n  %v = call i16 @leaf(i16 %x)\n  ret i16 %v\n}\n\n";
    if checked {
        text += "!0 = !{!\"branch_weights\", i32 1048575, i32 1}\n";
    }
    text
}

/// Whether `@main` still calls `@leaf` after the speed pipeline on this machine.
fn calls_leaf(machine: Dos, checked: bool) -> bool {
    let layout = llrm_x86_m16::layout();
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{}\"\n{}", layout.datalayout, source(checked))).expect("parses");
    let machine: Rc<dyn Machine> = Rc::new(machine);
    let mut program = Program::new(vec![module], machine).expect("program");
    applied(&mut program, &Applied { options: Options::speed(), only: None, dump: None }).expect("compiles");
    llrm_mir::print::module(&program.modules[0]).lines().any(|line| line.contains("call") && line.contains("@leaf("))
}

/// Without checks the call is hot and the local function goes in (the premise: the pin's inliner takes it on every tuning).
#[test]
fn a_call_after_unchecked_loop_nests_is_inlined() {
    assert!(!calls_leaf(Dos::pin_tuned(), false));
    assert!(!calls_leaf(Dos::default(), false));
}

/// The pin's tuning keeps the call (cold by the leaked mass, threshold 45); m16's does not: 3666 -> 3604 B, jpeg_single nib -O1.
#[test]
fn a_call_after_checked_loop_nests_is_inlined_on_m16_but_not_under_the_pin() {
    assert!(calls_leaf(Dos::pin_tuned(), true), "the pin's verdict");
    assert!(!calls_leaf(Dos::default(), true), "left a call of a 40-operation function held to threshold 45");
}
