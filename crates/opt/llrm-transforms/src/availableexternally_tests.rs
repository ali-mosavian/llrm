use llrm_mir::passes::PassManager;
use llrm_mir::program::Program;

use super::availableexternally::EliminateAvailableExternally;
use crate::testing::{parsed, printed};

/// An `available_externally` body left after inlining was emitted as a routine of the program, a
/// second copy of the runtime's own: it is a declaration again, and a call of it stays a call.
#[test]
fn a_body_held_to_inline_from_is_a_declaration_after() {
    let text = "define available_externally i16 @rt(i16 %x) {\nb:\n  ret i16 %x\n}\n\ndefine i16 @f(i16 %x) {\nb:\n  %v = call i16 @rt(i16 %x)\n  ret i16 %v\n}\n";
    let mut program = Program::new(vec![parsed(text)], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let mut manager = PassManager::default();
    manager.add_program(EliminateAvailableExternally);
    manager.run(&mut program).unwrap();
    let out = printed(&program.modules[0]);
    assert!(out.contains("declare i16 @rt(i16)"), "{out}");
    assert!(!out.contains("available_externally") && out.contains("call i16 @rt"), "{out}");
}
