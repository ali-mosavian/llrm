//! An `alloca` asking more than the target's frames keep is clamped to what
//! they keep, one asking more than it accepts is refused (`Machine::
//! kept_stack_align`, `max_alloca_align`).

use std::rc::Rc;

use llrm_mir::program::Program;
use llrm_mir::spaces::Spaces;
use llrm_mir::target::{AddressForm, Machine, Neutral, OperationCosts};
use llrm_transforms::pipeline::{Applied, Options, applied};

/// Frames keep 4, objects up to 8 are accepted.
struct Kept4Max8;

impl Machine for Kept4Max8 {
    fn foreign_span(
        &self,
        a: (i64, i64),
        b: (i64, i64),
        c: i64,
    ) -> Option<(i64, i64)> {
        Neutral.foreign_span(a, b, c)
    }
    fn spaces(&self) -> Spaces {
        Neutral.spaces()
    }
    fn costs(&self) -> OperationCosts {
        Neutral.costs()
    }
    fn registers(&self) -> i64 {
        Neutral.registers()
    }
    fn call_registers(&self) -> i64 {
        Neutral.call_registers()
    }
    fn address_forms(&self) -> Vec<AddressForm> {
        Neutral.address_forms()
    }
    fn kept_stack_align(&self) -> Option<u64> {
        Some(4)
    }
    fn max_alloca_align(&self) -> Option<u64> {
        Some(8)
    }
}

fn compile(
    body: &str,
    target: Rc<dyn Machine>,
) -> Result<String, String> {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"e-p:32:32-i64:32-f64:32\"\n{body}"))
        .map_err(|error| error.to_string())?;
    let mut program = Program::new(vec![module], target)?;
    let applied_options = Applied { options: Options::speed(), only: None, dump: None };
    applied(&mut program, &applied_options)?;
    Ok(llrm_mir::print::module(&program.modules[0]))
}

const MASKED: &str = "define i32 @f() {\n  %a = alloca double, align 8\n  store double 1.0, ptr %a\n  %i = ptrtoint ptr %a to i32\n  %m = and i32 %i, 7\n  ret i32 %m\n}\n";

/// `alloca double, align 8` on a target whose frames keep 4: `ptr & 7` folded
/// to 0 (the promised alignment), the frame gave an address ending in 4.
#[test]
fn an_object_between_kept_and_accepted_is_not_assumed_aligned() {
    let text = compile(MASKED, Rc::new(Kept4Max8)).expect("compiles");
    assert!(!text.contains("ret i32 0"), "{text}");
}

/// The control: where the frame keeps the 8, the fold is right and happens.
#[test]
fn the_control_folds_where_the_frame_keeps_the_alignment() {
    let text = compile(MASKED, Rc::new(Neutral)).expect("compiles");
    assert!(text.contains("ret i32 0"), "{text}");
}

#[test]
fn an_object_past_the_accepted_alignment_is_refused() {
    let why = compile(&MASKED.replace("align 8", "align 16"), Rc::new(Kept4Max8)).expect_err("refused");
    assert!(why.contains("@f: an object aligned to 16 needs stack realignment"), "{why}");
}

const DYNAMIC: &str = "define i32 @f(i32 %n) {\n  %a = alloca i8, i32 %n, align 8\n  store i8 1, ptr %a\n  %i = ptrtoint ptr %a to i32\n  ret i32 %i\n}\n";

#[test]
fn a_dynamic_object_is_clamped_and_refused_by_the_same_bands() {
    let text = compile(DYNAMIC, Rc::new(Kept4Max8)).expect("compiles");
    assert!(text.contains("alloca i8, i32 %n, align 4"), "{text}");
    let why = compile(&DYNAMIC.replace("align 8", "align 16"), Rc::new(Kept4Max8)).expect_err("refused");
    assert!(why.contains("aligned to 16 needs stack realignment"), "{why}");
}
