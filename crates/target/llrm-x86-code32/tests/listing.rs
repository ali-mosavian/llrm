//! `int add(int, int)` and a loop over `int *`, as flat 32-bit code.

use llrm_core::abi::qb::HirAbi;
use llrm_core::backend::{assemble, masm, target};
use llrm_core::backend::cpu::ProfileOrName;
use llrm_core::backend::isel;
use llrm_target::Target;
use llrm_x86_code32::Code32;

const LAYOUT: &str = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-n8:16:32\"\n";

fn listing(text: &str) -> String {
    let module = llrm_mir::parse::module(&format!("{LAYOUT}{text}")).expect("parses");
    let abi = HirAbi { runtime: llrm_core::hir::model::RuntimeProfile::Freestanding, objects: Default::default(), preserved: Default::default(), stack_check: None };
    let segments = target::Segments::of(&Code32.machine());
    let selection = isel::selector("x86-code32").expect("selector");
    let assembled = assemble::assembled_by(&module, &abi, "T_TEXT", ProfileOrName::Name("486"), &segments, selection, &Code32).expect("assembles");
    masm::text(&assembled).expect("prints")
}

#[test]
fn explore_add() {
    println!("{}", listing("define i32 @add(i32 %a, i32 %b) {\nentry:\n  %c = add i32 %a, %b\n  ret i32 %c\n}\n"));
}
