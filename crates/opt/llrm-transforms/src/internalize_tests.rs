use llrm_mir::passes::PassManager;
use llrm_mir::program::Program;

use super::internalize::Internalize;
use crate::globaldce::GlobalDce;
use crate::testing::{parsed, printed};

const PROGRAM: &str = "@table = global [2 x i16] [i16 1, i16 2]
@unused = global i16 7
@kept = global i16 9
@local = internal global i16 5
declare i16 @outside()

define i16 @helper() {
b:
  %v = load i16, ptr @table
  ret i16 %v
}

define i16 @orphan() {
b:
  %v = load i16, ptr @unused
  %w = call i16 @outside()
  ret i16 %v
}

define i16 @main() {
b:
  %v = call i16 @helper()
  ret i16 %v
}
";

fn swept(dce: bool) -> String {
    let mut program = Program::new(vec![parsed(PROGRAM)], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    program.exports.kept = ["kept".to_owned()].into();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add_program(Internalize::new(["main".to_owned()]));
    if dce {
        manager.add_program(GlobalDce);
    }
    manager.run(&mut program).unwrap();
    printed(&program.modules[0])
}

/// `float_return` at -O2 kept `scale` and `narrow` and the soft-float they pull
/// (5945 B against llvm-mos's 90): an external function nothing calls is not
/// deletable until the program says it is the whole link.
#[test]
fn test_a_definition_that_is_not_preserved_becomes_internal() {
    let text = swept(false);
    assert!(text.contains("define internal i16 @helper()"), "{text}");
    assert!(text.contains("define internal i16 @orphan()"), "{text}");
    assert!(text.contains("@table = internal global"), "{text}");
}

#[test]
fn test_the_preserved_names_the_declarations_and_the_kept_stay_external() {
    let text = swept(false);
    assert!(text.contains("define i16 @main()"), "{text}");
    assert!(text.contains("declare i16 @outside()"), "{text}");
    assert!(text.contains("@kept = global"), "{text}");
}

/// Global DCE then deletes what no live body names.
#[test]
fn test_what_nothing_names_goes_with_the_next_global_dce() {
    let text = swept(true);
    assert!(!text.contains("@orphan") && !text.contains("@unused"), "{text}");
    assert!(text.contains("@helper") && text.contains("@table"), "{text}");
}
