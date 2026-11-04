use std::sync::atomic::Ordering;

use llrm_codegen::backend::selectiondag::function_lowering::MODULE_TABLES_BUILT;
use llrm_wasm::Width;
use llrm_wasm::dag::{MODULE_FACTS_BUILT, Options};

/// What each callee does to memory and which callees never throw were built by
/// a scan of the whole module for every function compiled: N functions cost
/// N^2 (5.5 G instructions for 600 tiny functions, 15.7 G for 1200, where the
/// pin is linear). The builder also made the names of the module's globals
/// for every function (435 M of 7.4 G at 1200). Once per module is once for any N.
#[test]
fn the_module_facts_and_tables_are_built_once_per_module_not_once_per_function() {
    let mut text = String::from(
        "target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20\"\ntarget triple = \"wasm32-unknown-unknown\"\n\ndeclare i32 @h(i32)\n\n",
    );
    for k in 0..40 {
        text.push_str(&format!("define i32 @f{k}(i32 %a) {{\n  %r = call i32 @h(i32 %a)\n  ret i32 %r\n}}\n\n"));
    }
    let (before, tables_before) = (MODULE_FACTS_BUILT.load(Ordering::Relaxed), MODULE_TABLES_BUILT.load(Ordering::Relaxed));
    llrm_wasm::driver::object(&text, Width::W32, Options::default()).expect("compiles");
    let built = MODULE_FACTS_BUILT.load(Ordering::Relaxed) - before;
    let tables = MODULE_TABLES_BUILT.load(Ordering::Relaxed) - tables_before;
    assert_eq!((built, tables), (1, 1), "facts built {built} times, tables {tables} times, for 40 functions");
}
