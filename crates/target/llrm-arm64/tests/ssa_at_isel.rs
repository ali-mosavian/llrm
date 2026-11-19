//! The selector's output is SSA: a loop-carried value is one phi, not a copy
//! into the same register on each path. The selector made `i` below a
//! register defined twice (an `orr` on the entry edge, another on the back
//! edge), so the shared SSA phases, which read one definition per value, could
//! not run on arm64 (184 such registers in the -O1 corpus).

use std::collections::HashMap;

use llrm_arm64::dag::emit::emit;
use llrm_arm64::dag::function::select_function;
use llrm_arm64::mfunc::MFunc;

const DATALAYOUT: &str = "e-m:o-i64:64-i128:128-n32:64-S128";

const LOOP: &str = "define i64 @f(i64 %n) {\n\
entry:\n  br label %head\n\
head:\n  %i = phi i64 [ 0, %entry ], [ %next, %head ]\n  %next = add i64 %i, 1\n  %done = icmp eq i64 %next, %n\n  br i1 %done, label %exit, label %head\n\
exit:\n  ret i64 %next\n}";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{LOOP}")).expect("parses")
}

/// The virtual registers defined more than once.
fn defined_twice(function: &MFunc) -> Vec<String> {
    let mut count: HashMap<String, usize> = HashMap::new();
    for op in function.blocks.iter().flat_map(|block| block.ops.iter()) {
        for register in op.defs_and_uses().0.into_iter().filter(|register| register.is_virtual()) {
            *count.entry(format!("{register:?}")).or_default() += 1;
        }
    }
    let mut out: Vec<String> = count.into_iter().filter(|(_, n)| *n > 1).map(|(register, _)| register).collect();
    out.sort();
    out
}

#[test]
fn the_dag_selectors_loop_carried_value_has_one_definition() {
    let function = emit("f", &select_function(&module(), "f").expect("selects"), false).expect("emits");
    assert_eq!(defined_twice(&function), Vec::<String>::new(), "{function:?}");
}
