//! `va_start`, `va_end` and `va_copy` build `vastart`, `vaend` and `vacopy`
//! with a `SrcValue` per list, on the root, as the pin's initial DAGs do.
//! Before, each stopped the builder at "intrinsic call".

use super::bundle_tests::{ours, pins};

#[test]
fn va_intrinsics_match_the_pins_initial_dags() {
    let (ours, pins) = (ours(include_str!("golden/vararg.ll")), pins(include_str!("golden/vararg.pin.txt")));
    assert_eq!(ours.keys().collect::<Vec<_>>(), pins.keys().collect::<Vec<_>>());
    for (name, dumps) in &ours {
        assert_eq!(dumps, &pins[name], "function {name}");
    }
}

/// `va_start` stores the address of the first variadic argument, the fixed
/// object after the named ones: -2 for `sum(int n, ...)`. The builder left
/// it unset, so the lowering of `vastart` had nothing to store.
#[test]
fn a_variadic_function_names_its_first_variadic_argument() {
    use super::builder::{BuilderTarget, build_block};
    use super::function_lowering::FunctionLoweringInfo;
    let text: String = include_str!("golden/vararg.ll")
        .lines()
        .filter(|line| !line.starts_with("target triple"))
        .map(|line| format!("{line}\n"))
        .collect();
    let module = llrm_mir::parse::module(&text).expect("parses");
    let layout = llrm_mir::datalayout::DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (id, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("sum");
    let convention = crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
    let mut info = FunctionLoweringInfo::new(function);
    let entry = function.entry().expect("an entry");
    let done = build_block(
        &module,
        function,
        &layout,
        &BuilderTarget::m32(),
        &llrm_x86_m32::M32,
        &convention,
        &mut info,
        entry,
    )
    .expect("builds");
    assert_eq!(done.dag.var_args_frame_index, Some(-2));
}
