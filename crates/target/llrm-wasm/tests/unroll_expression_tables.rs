//! A partial unroll's `simplifyLoopIVs` and `loadCSE` ask one table of
//! expressions, as the pin's do: each began with a table and a session of its
//! own (the sorted-check loop of bench/quicksort, 1023 trips, three copies).

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, object};

fn module_of(functions: usize) -> String {
    let mut ir = String::from(
        "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"\n\
         target triple = \"wasm32-unknown-unknown\"\n@a = global [1024 x i32] zeroinitializer, align 16\n",
    );
    for n in 0..functions {
        ir.push_str(&format!(
            "define i32 @sorted{n}() {{\nentry:\n  br label %loop\n\nloop:\n  %i = phi i32 [ 1, %entry ], [ %next, %cont ]\n  \
             %p = getelementptr [4 x i8], ptr @a, i32 %i\n  %q = getelementptr i8, ptr %p, i32 -4\n  %x = load i32, ptr %q\n  \
             %y = load i32, ptr %p\n  %bad = icmp sgt i32 %x, %y\n  br i1 %bad, label %fail, label %cont\n\ncont:\n  \
             %next = add nuw nsw i32 %i, 1\n  %done = icmp eq i32 %next, 1024\n  br i1 %done, label %ok, label %loop\n\n\
             fail:\n  ret i32 -1\n\nok:\n  ret i32 0\n}}\n"
        ));
    }
    ir
}

/// (sessions, tables) one function costs the compile, from a module of 4 and
/// a module of 8.
fn per_function() -> (usize, usize) {
    let made = |functions: usize| {
        let (sessions, tables) = (llrm_analysis::scev::analysis::sessions_made(), llrm_analysis::scev::made());
        let options = Options { level: Level::O2, ..Options::default() };
        object(&module_of(functions), Width::W32, options).expect("compiles");
        (llrm_analysis::scev::analysis::sessions_made() - sessions, llrm_analysis::scev::made() - tables)
    };
    let (four, eight) = (made(4), made(8));
    ((eight.0 - four.0) / 4, (eight.1 - four.1) / 4)
}

/// Before: 7 sessions and 35 tables per function at -O2; the partial unroll's
/// IV pass made a session and a table of its own and `loadCSE` another table.
#[test]
fn a_partial_unroll_keeps_one_session_and_one_expression_table() {
    let (sessions, tables) = per_function();
    assert!(
        sessions <= 6 && tables <= 33,
        "{sessions} sessions and {tables} tables per function; the ceiling is 6 and 33"
    );
}
