//! `CodeMetrics` on blocks whose numbers are worked out by hand at `-mcpu=i486`
//! `CodeSize`: `add`, `icmp`, `load`, `br` cost 1; `llvm.assume` and a GEP
//! with a legal address cost 0.

use llrm_mir::codemetrics::{CodeMetrics, Convergence, Query, collect_ephemeral_values};
use llrm_mir::context::GlobalId;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;

use crate::tti::X86;

const DATALAYOUT: &str = "target datalayout = \"e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128\"\n";

fn module(text: &str) -> Module {
    llrm_mir::parse::module(&format!("{DATALAYOUT}{text}")).expect("parses")
}

/// The metrics of block `b` of `@name`, with or without the ephemeral values.
fn metrics(
    module: &Module,
    name: &str,
    ephemeral: bool,
) -> CodeMetrics {
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let x86 = X86::of_cpu("i486").unwrap();
    let (id, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some(name)).unwrap();
    let block =
        function.layout().iter().copied().find(|&one| function.block(one).name.as_deref() == Some("b")).unwrap();
    let assumes: Vec<_> = function
        .layout()
        .iter()
        .flat_map(|&one| function.iter_block(one))
        .filter(|&inst| {
            llrm_mir::memory::callee(&module.context, function, inst)
                .is_some_and(|callee| module.global(callee).name.as_deref() == Some("llvm.assume"))
        })
        .collect();
    let q = Query {
        tti: &x86,
        context: &module.context,
        layout: &layout,
        globals: &module.globals,
        this: Some(id),
        has_one_live_use: &|_: GlobalId| true,
    };
    let eph = if ephemeral {
        collect_ephemeral_values(function, &|_| true, &assumes, &|_| true)
    } else {
        llrm_mir::dense::IdSet::new()
    };
    let mut found = CodeMetrics::default();
    found.analyze_basic_block(&q, function, block, &eph, false);
    found
}

const ASSUMING: &str = "declare void @llvm.assume(i1)
define void @f(ptr %p, i16 %a, i16 %i) {
b:
  %s = add i16 %a, %i
  %c = icmp slt i16 %s, 9
  call void @llvm.assume(i1 %c)
  %g = getelementptr i16, ptr %p, i16 %i
  %l = load i16, ptr %g
  br label %b
}
";

/// The condition only an `assume` uses is ephemeral, and so is the add that
/// only the condition uses: 4 with the add and the compare, 2 (the load and the
/// branch) without them.
#[test]
fn test_values_only_an_assume_uses_are_not_counted() {
    let module = module(ASSUMING);
    assert_eq!(metrics(&module, "f", false).num_insts.value, 4);
    assert_eq!(metrics(&module, "f", true).num_insts.value, 2);
}

#[test]
fn test_a_dynamic_alloca_a_call_to_itself_and_a_convergent_call_are_noted() {
    let module = module(
        "declare void @conv() convergent
define void @f(i32 %n) {
b:
  %a = alloca i32, i32 %n
  call void @f(i32 %n)
  call void @conv()
  br label %b
}
",
    );
    let found = metrics(&module, "f", false);
    assert!(found.uses_dynamic_alloca && found.is_recursive && !found.not_duplicatable);
    assert_eq!(found.convergence, Convergence::Uncontrolled);
    assert_eq!(found.num_inline_candidates, 0);
}

#[test]
fn test_an_internal_function_with_one_use_is_an_inline_candidate() {
    let module = module(
        "define internal void @leaf() {
e:
  ret void
}
define void @f() {
b:
  call void @leaf()
  ret void
}
",
    );
    let found = metrics(&module, "f", false);
    assert_eq!((found.num_inline_candidates, found.num_rets), (1, 1));
}
