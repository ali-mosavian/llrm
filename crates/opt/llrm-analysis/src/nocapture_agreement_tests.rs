//! The `nocapture` that `add_argument_attrs` infers for a pointer parameter
//! against `capturecheck`'s independent reader of the same body
//! (`Verify::parameter`): AA's call ModRef takes a parameter's `nocapture` as
//! the callee's promise, so the two must say the same of every parameter.

use llrm_mir::facts::Facts;
use llrm_mir::module::Module;

use crate::capturecheck::Verify;
use crate::testing::{DOS, corpus, layout, parsed};

/// Each pointer parameter of each defined function of `module`, after
/// attributes are inferred: (`@f`/index, `nocapture` now, whether the frontend
/// stated it in the text, shown uncaptured by the body). A stated `nocapture`
/// (Nib's by-reference parameters, which it promises of a callee it cannot see
/// into) is the frontend's, not inferred.
fn parameters(module: &mut Module) -> Vec<(String, bool, bool, bool)> {
    let stated: Vec<Vec<bool>> = module
        .functions()
        .map(|(_, _, function)| function.parameter_attrs.iter().map(|attrs| Facts::of(attrs).no_capture()).collect())
        .collect();
    let layout = layout(module);
    crate::functionattrs::infer_module(module, &layout, llrm_mir::target::Machine::spaces(&llrm_mir::target::Neutral));
    let verify = Verify::new(module, &layout, 8);
    let mut found = Vec::new();
    for ((id, global, function), stated) in module.functions().zip(stated) {
        if function.is_declaration() {
            continue;
        }
        for (index, &parameter) in function.parameters().iter().enumerate() {
            let is_pointer =
                matches!(
                    module.context.types.get(function.value(parameter).ty),
                    llrm_mir::types::Type::Pointer(_)
                );
            if !is_pointer {
                continue;
            }
            let inferred = Facts::of(&function.parameter_attrs[index]).no_capture();
            let shown = verify.parameter(id, index).is_ok();
            found.push((format!("@{}/{index}", global.name.as_deref().unwrap_or("")), inferred, stated[index], shown));
        }
    }
    found
}

/// A parameter is `nocapture` exactly when its body keeps no copy of it, over
/// functions that load through it, store through it, store it, return it, pass
/// it on to a callee that keeps it and to one that does not, and take its
/// address into an integer.
#[test]
fn an_inferred_nocapture_is_a_parameter_the_body_does_not_keep() {
    let mut module = parsed(&format!(
        "{DOS}@g = global ptr null

declare void @keeps(ptr)

define i16 @load(ptr %p) {{
b0:
  %v = load i16, ptr %p
  ret i16 %v
}}

define void @store_through(ptr %p) {{
b0:
  store i16 1, ptr %p
  ret void
}}

define void @store_it(ptr %p) {{
b0:
  store ptr %p, ptr @g
  ret void
}}

define ptr @return_it(ptr %p) {{
b0:
  ret ptr %p
}}

define i16 @to_int(ptr %p) {{
b0:
  %n = ptrtoint ptr %p to i16
  ret i16 %n
}}

define void @pass_to_unknown(ptr %p) {{
b0:
  call void @keeps(ptr %p)
  ret void
}}

define i16 @inner(ptr %p) {{
b0:
  %v = load i16, ptr %p
  ret i16 %v
}}

define i16 @pass_to_known(ptr %p) {{
b0:
  %v = call i16 @inner(ptr %p)
  ret i16 %v
}}

define i16 @through_gep_and_phi(ptr %p, i1 %c) {{
b0:
  %q = getelementptr i16, ptr %p, i16 1
  br i1 %c, label %a, label %b
a:
  br label %m
b:
  br label %m
m:
  %r = phi ptr [ %p, %a ], [ %q, %b ]
  %v = load i16, ptr %r
  ret i16 %v
}}
"
    ));
    let found = parameters(&mut module);
    let named = |name: &str| found.iter().find(|(at, _, _, _)| at == name).unwrap_or_else(|| panic!("{name}"));
    // The premise: the readings differ across the set.
    assert!(found.iter().any(|(_, inferred, _, _)| *inferred) && found.iter().any(|(_, inferred, _, _)| !*inferred));
    for (at, inferred, _, shown) in &found {
        assert_eq!(inferred, shown, "{at}: inferred nocapture {inferred}, the capture check shows {shown}");
    }
    assert!(named("@load/0").1 && named("@pass_to_known/0").1 && named("@through_gep_and_phi/0").1);
    assert!(!named("@store_it/0").1 && !named("@return_it/0").1 && !named("@pass_to_unknown/0").1);
}

/// The same over every module of the corpus (the compiles' own bodies): a
/// `nocapture` that is inferred is one the capture check shows (the direction a
/// call's ModRef relies on). The other direction differs only where the pin's
/// capture tracking counts a store of the pointer into a local slot as a
/// capture and the check reads it back through the slot: `@floats.digits` of
/// the nib runtime, whose parameter is spilled to a local in the emitted
/// stage, before mem2reg, and `@$state3.next` of the pipeline example in the
/// optimized stage (an irreducible body, `corpus_tests`). Missing a
/// `nocapture` is the safe side. A `nocapture` the frontend states (Nib's
/// by-reference parameters) is its promise, not inferred, and is not asked of
/// the check.
#[test]
fn every_inferred_nocapture_in_the_corpus_is_shown_by_the_capture_check() {
    let mut unsound = Vec::new();
    let mut unstated = Vec::new();
    let mut parameters_seen = 0;
    for (name, mut module) in corpus() {
        for (at, inferred, stated, shown) in parameters(&mut module) {
            parameters_seen += 1;
            if inferred && !stated && !shown {
                unsound.push(format!("{name}/{at}"));
            }
            if !inferred && shown {
                unstated.push(format!("{name}/{at}"));
            }
        }
    }
    assert!(parameters_seen > 500, "the corpus has pointer parameters: {parameters_seen}");
    assert!(unsound.is_empty(), "inferred nocapture the body keeps: {unsound:#?}");
    assert_eq!(
        unstated,
        ["emitted/nib-runtime/@floats.digits/0", "optimized/nib-examples-pipeline/@$state3.next/0"],
        "shown uncaptured, not stated"
    );
}
