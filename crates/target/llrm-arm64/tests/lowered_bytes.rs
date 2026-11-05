//! `Tti::lowered_bytes`: the trial of an unroll measures the copy as the
//! backend makes it. The IR model counted one instruction a store and a load
//! and no spill: lru -O3 modelled +8 B for a loop whose unrolled stores the
//! backend merges, nbody_single -O3 modelled -112 B for a 6x6 unroll that
//! spilled (+460 B built).

use std::rc::Rc;

use llrm_arm64::machine::{Arm64Machine, Platform};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::target::Machine;
use llrm_mir::tti::{Basic, LoweredFunction, Tti};

/// The lowered bytes of `@f` in `text` under `tti`, and its IR instruction count.
fn lowered(
    tti: &dyn Tti,
    text: &str,
) -> (Option<i64>, usize) {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{}\"\n{text}", Platform::Linux.datalayout())).expect("parses");
    let layout = DataLayout::parse(module.datalayout.as_deref().expect("a datalayout")).expect("layout");
    let (id, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let declarations: Vec<_> = module.globals.iter().cloned().collect();
    let count = function.walk().count();
    let request = LoweredFunction {
        context: &module.context,
        globals: &declarations,
        metadata: &module.metadata,
        layout: &layout,
        function,
        id,
    };
    (tti.lowered_bytes(&request), count)
}

fn arm64() -> Rc<dyn Tti> {
    Arm64Machine { platform: Platform::Linux }.tti().expect("a tti")
}

#[test]
fn a_target_without_a_lowering_leaves_the_ir_model_in_charge() {
    let (bytes, _) = lowered(&Basic, "define void @f() {\n  ret void\n}\n");
    assert_eq!(bytes, None);
}

/// Eight adjacent halfword stores of -1: eight stores in the IR, a few merged
/// ones in the code.
#[test]
fn adjacent_constant_stores_cost_what_the_backend_merges_them_to() {
    let stores: String = (0..8)
        .map(|at| format!("  store i16 -1, ptr getelementptr inbounds (i8, ptr @a, i64 {})\n", at * 2))
        .collect();
    let text = format!("@a = global [8 x i16] zeroinitializer\ndefine void @f() {{\n{stores}  ret void\n}}\n");
    let (bytes, count) = lowered(&*arm64(), &text);
    let bytes = bytes.expect("lowers");
    assert!(bytes < 4 * count as i64, "{bytes} B for {count} IR instructions");
}

/// Forty floats summed: loaded first, all live at once, or loaded one beside
/// each add. The same IR instructions; the first spills.
#[test]
fn many_live_floats_cost_their_spills() {
    let load = |at: usize| format!("  %p{at} = getelementptr inbounds float, ptr @a, i64 {at}\n  %v{at} = load float, ptr %p{at}\n");
    let add = |at: usize| format!("  %s{at} = fadd float %s{}, %v{}\n", at - 1, at + 1);
    let body = |together: bool| {
        let mut text = String::new();
        if together {
            (0..40).for_each(|at| text.push_str(&load(at)));
            text.push_str("  %s0 = fadd float %v0, %v1\n");
            (1..39).for_each(|at| text.push_str(&add(at)));
        } else {
            text.push_str(&load(0));
            text.push_str(&load(1));
            text.push_str("  %s0 = fadd float %v0, %v1\n");
            for at in 1..39 {
                text.push_str(&load(at + 1));
                text.push_str(&add(at));
            }
        }
        format!("@a = global [64 x float] zeroinitializer\ndefine float @f() {{\n{text}  ret float %s38\n}}\n")
    };
    let spread = lowered(&*arm64(), &body(false));
    let together = lowered(&*arm64(), &body(true));
    assert_eq!(spread.1, together.1, "the same IR instructions");
    assert!(together.0.expect("lowers") > spread.0.expect("lowers"), "{together:?} against {spread:?}");
}
