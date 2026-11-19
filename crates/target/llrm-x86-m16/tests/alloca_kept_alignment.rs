//! m16 frames keep 2 bytes (`kept_stack_align`) and accept an `alloca` up to 32 (`max_alloca_align`): the optimizer does not assume the
//! alignment a frame does not give, and an object past 32 is refused by name (KNOWN-WRONG-ALIGN-M16).

use std::rc::Rc;

use llrm_mir::program::Program;
use llrm_mir::target::Machine;
use llrm_transforms::pipeline::{Applied, Options, applied};

fn compile(body: &str) -> Result<String, String> {
    let layout = llrm_x86_m16::layout();
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{}\"\n{body}", layout.datalayout))
        .map_err(|error| error.to_string())?;
    let machine: Rc<dyn Machine> = Rc::new(llrm_x86_m16::target::Dos::default());
    let mut program = Program::new(vec![module], machine)?;
    applied(&mut program, &Applied { options: Options::speed(), only: None, dump: None })?;
    Ok(llrm_mir::print::module(&program.modules[0]))
}

const MASKED: &str = "define i16 @f() {\n  %a = alloca double, align 8\n  store double 1.0, ptr %a, align 2\n  %i = ptrtoint ptr %a to i16\n  %m = and i16 %i, 7\n  ret i16 %m\n}\n";

/// `alloca double, align 8` stored at align 2: `ptr & 7` folded to 0 (the promised 8), the frame gave an even address.
#[test]
fn an_aligned_double_is_not_assumed_aligned_past_what_the_frame_keeps() {
    let text = compile(MASKED).expect("compiles");
    assert!(!text.contains("ret i16 0"), "{text}");
}

#[test]
fn an_object_past_the_accepted_alignment_is_refused_by_name() {
    let why = compile(&MASKED.replace("align 8", "align 64")).expect_err("refused");
    assert!(why.contains("@f: an object aligned to 64 needs stack realignment"), "{why}");
}

#[test]
fn an_object_at_the_accepted_alignment_is_taken() {
    compile(&MASKED.replace("align 8", "align 32")).expect("compiles");
}
