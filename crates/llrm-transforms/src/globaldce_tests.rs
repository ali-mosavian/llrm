use llrm_mir::passes::PassManager;
use llrm_mir::program::Program;

use super::globaldce::GlobalDce;
use crate::testing::{parsed, printed};

const DEAD: &str = "@table = internal global [2 x i16] [i16 1, i16 2]
@unused = internal global i16 7
@kept = internal global i16 9
declare i16 @outside()

define internal i16 @helper() {
b:
  %v = load i16, ptr @table
  ret i16 %v
}

define internal i16 @orphan() {
b:
  %v = load i16, ptr @unused
  %w = call i16 @outside()
  ret i16 %v
}

define internal i16 @entered() {
b:
  ret i16 3
}

define i16 @f() {
b:
  %v = call i16 @helper()
  ret i16 %v
}
";

fn swept(text: &str) -> String {
    let mut program = Program::new(vec![parsed(text)], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    program.exports.entries = ["entered".to_owned()].into();
    program.exports.kept = ["kept".to_owned()].into();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add_program(GlobalDce);
    manager.run(&mut program).unwrap();
    printed(&program.modules[0])
}

/// An internal body and variable nothing live names went on to isel and
/// into the object.
#[test]
fn test_what_nothing_live_names_is_deleted() {
    let text = swept(DEAD);
    assert!(!text.contains("@orphan") && !text.contains("@unused"), "{text}");
    assert!(text.contains("define internal i16 @helper()") && text.contains("load i16, ptr @table"), "{text}");
}

/// Deleting renumbers the rest; an entry, a kept variable and a
/// declaration stay whether or not anything names them.
#[test]
fn test_roots_and_declarations_stay() {
    let text = swept(DEAD);
    for one in ["@entered", "@kept", "declare i16 @outside()", "call i16 @helper()"] {
        assert!(text.contains(one), "{one}\n{text}");
    }
}
