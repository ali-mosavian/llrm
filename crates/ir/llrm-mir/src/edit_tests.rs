use crate::module::{Change, Function, InstId, Module, Operand, Use, ValueId};
use crate::opcode::{BinaryOp, Flags, Opcode};
use crate::{Position, parse, print};

const TEXT: &str = "define i16 @f(i16 %a) {\nentry:\n  %x = add i16 %a, 1\n  %y = mul i16 %x, 2\n  ret i16 %y\n}\n";

fn module(text: &str) -> Module {
    parse::module(text).unwrap_or_else(|error| panic!("{error}"))
}

fn function(module: &mut Module) -> &mut Function {
    module.function_mut("f").expect("@f").1
}

/// The instruction defining `name`, and its value.
fn named(function: &Function, name: &str) -> (InstId, ValueId) {
    function
        .walk()
        .find_map(|(_, inst)| {
            let result = function.instruction(inst).result?;
            (function.value(result).name.as_deref() == Some(name)).then_some((inst, result))
        })
        .unwrap_or_else(|| panic!("%{name}"))
}

#[test]
fn replacing_every_use_lets_the_definition_go() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (x, value) = named(f, "x");
    let (y, _) = named(f, "y");
    assert_eq!(f.users(value), [Use { user: y, index: 0 }]);
    assert_eq!(f.erase(x), Err(format!("instruction {}'s result still has 1 users", x.0)));
    let a = f.parameters()[0];
    f.replace_all_uses_with(value, Operand::Value(a));
    f.erase(x).expect("unused now");
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
    assert_eq!(f.take_changes(), [Change::Rewritten(y), Change::Erased { inst: x, block: f.entry().unwrap(), next: Some(y) }]);
    assert_eq!(print::module(&module), "define i16 @f(i16 %a) {\nentry:\n  %y = mul i16 %a, 2\n  ret i16 %y\n}\n");
}

#[test]
fn an_erased_id_is_never_reused() {
    let mut module = module(TEXT);
    let i16 = module.context.types.int(16);
    let one = module.context.int(i16, 1);
    let f = function(&mut module);
    let (y, value) = named(f, "y");
    let (x, _) = named(f, "x");
    let ret = f.terminator(f.entry().unwrap()).unwrap();
    let z = f.create_instruction(Opcode::Binary(BinaryOp::Sub), i16, vec![Operand::Value(value), Operand::Constant(one)], Flags::default(), Some("x"));
    assert!(z.0 > ret.0 && z.0 > x.0 && z.0 > y.0);
    f.insert(z, Position::Before(ret)).expect("placed");
    let result = f.instruction(z).result.unwrap();
    f.set_operand(ret, 0, Operand::Value(result));
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
    assert!(print::module(&module).contains("  %x1 = sub i16 %y, 1\n  ret i16 %x1\n"), "{}", print::module(&module));
}

#[test]
fn a_clone_uses_what_its_original_uses() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (x, value) = named(f, "x");
    let (y, _) = named(f, "y");
    let copy = f.clone_instruction(y);
    f.insert(copy, Position::Before(y)).expect("placed");
    assert_eq!(f.users(value).len(), 2);
    assert_eq!(f.take_changes()[0], Change::Cloned { from: y, to: copy });
    assert_eq!(f.erase(x), Err(format!("instruction {}'s result still has 2 users", x.0)));
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
}

#[test]
fn blocks_split_by_moving_and_renaming_their_uses() {
    let text = "define i16 @f(i1 %c) {\nentry:\n  br i1 %c, label %yes, label %no\n\nyes:\n  ret i16 1\n\nno:\n  ret i16 2\n}\n";
    let mut module = module(text);
    let f = function(&mut module);
    let entry = f.entry().unwrap();
    let no = f.successors(entry)[1];
    let middle = f.create_block(Some("no"));
    f.insert_block(middle, Some(entry)).expect("placed");
    f.replace_block_uses_with(no, middle);
    let ret = f.terminator(no).unwrap();
    f.move_to(ret, Position::End(middle)).expect("moved");
    f.erase_block(no).expect("empty and unnamed by any terminator");
    assert_eq!(f.predecessors(middle), [entry]);
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
    assert_eq!(
        print::module(&module),
        "define i16 @f(i1 %c) {\nentry:\n  br i1 %c, label %yes, label %no1\n\nno1:\n  ret i16 2\n\nyes:\n  ret i16 1\n}\n"
    );
}

#[test]
fn the_use_list_check_notices_an_operand_changed_behind_its_back() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (y, _) = named(f, "y");
    let a = f.parameters()[0];
    f.instructions[y.0 as usize].operands[0] = Operand::Value(a);
    assert_eq!(f.check_uses().len(), 2, "{:?}", f.check_uses());
}

/// A refused body goes whole, and what calls the function still verifies.
#[test]
fn deleting_a_body_leaves_a_declaration() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    f.delete_body();
    assert!(f.check_uses().is_empty(), "{:?}", f.check_uses());
    assert!(f.is_declaration());
    assert_eq!(print::module(&module), "declare i16 @f(i16)\n");
}

/// A result derived at a mark is brought up to date from what changed after it, only while that is on record and of the
/// same function: a mark of the log a pass has taken, or of a copy's original, names changes no one can list.
#[test]
fn a_mark_names_the_changes_since_while_they_are_on_record_of_that_function() {
    let mut module = module(TEXT);
    let f = function(&mut module);
    let (x, value) = named(f, "x");
    let (y, _) = named(f, "y");
    let a = f.parameters()[0];
    let mark = f.mark();
    assert_eq!(f.changes_since(mark), Some(&[][..]));
    f.replace_all_uses_with(value, Operand::Value(a));
    assert_eq!(f.changes_since(mark), Some(&[Change::Rewritten(y)][..]));
    let mut copy = f.clone();
    assert_eq!(copy.changes_since(mark), None, "a copy's edits are its own");
    assert_eq!(copy.changes_since(copy.mark()), Some(&[][..]));
    f.erase(x).expect("unused now");
    assert_eq!(f.changes_since(mark).map(<[Change]>::len), Some(2));
    f.take_changes();
    assert_eq!(f.changes_since(mark), None, "taken from the log");
    assert_eq!(f.changes_since(f.mark()), Some(&[][..]));
    let _ = &mut copy;
}

/// A parameter the passes removed leaves the others named by the position they had: `-g` named `scale(p, factor)`'s `factor` the
/// second, and the function that kept only it made the first parameter `p`'s, a cell and a name that were not its own.
#[test]
fn a_parameter_keeps_the_position_it_was_named_by() {
    let mut parsed = module("define i16 @f(i16 %a, i16 %b, i16 %c) {\nentry:\n  %x = add i16 %b, 1\n  ret i16 %x\n}\n");
    let (context, f) = parsed.function_mut("f").expect("@f");
    assert_eq!((f.parameter_origin(0), f.parameter_origin(1)), (Some(0), Some(1)));
    f.remove_parameter(context, 2);
    f.remove_parameter(context, 0);
    assert_eq!(f.parameter_origin(0), Some(1));
    assert_eq!(f.parameter_origin(1), None);
    let i16 = context.types.int(16);
    f.insert_parameters(context, 0, &[i16]);
    assert_eq!((f.parameter_origin(0), f.parameter_origin(1)), (None, Some(1)));
}

const RECORDED: &str = "define i16 @f(i16 %a) {\nentry:\n  %p = alloca i16\n  #dbg_declare(ptr %p, !0)\n  %x = add i16 %a, 1\n  #dbg_value(i16 %x, !0)\n  %y = mul i16 %x, 2\n  #dbg_gone(!0)\n  ret i16 %y\n}\n\n!0 = !{!\"v\"}\n";

fn records(function: &Function) -> Vec<(u32, crate::DebugWhat)> {
    function.debug_records().iter().map(|one| (one.before.0, one.what)).collect()
}

/// What `-g` says of a variable stands before an instruction, as LLVM's debug records print: a declare, a value, and one that
/// says nothing; read back, it prints as it was, its variable the same node.
#[test]
fn debug_records_print_and_parse_as_they_were() {
    let mut parsed = module(RECORDED);
    assert_eq!(print::module(&parsed), RECORDED);
    let f = function(&mut parsed);
    let (x, value) = named(f, "x");
    let (y, _) = named(f, "y");
    let (p, pointer) = named(f, "p");
    let ret = f.terminator(f.entry().unwrap()).unwrap();
    assert_eq!(
        records(f),
        [(x.0, crate::DebugWhat::Declare(Operand::Value(pointer))), (y.0, crate::DebugWhat::Value(Operand::Value(value))), (ret.0, crate::DebugWhat::Gone)]
    );
    let _ = p;
}

/// A value another replaces is the other in the records that named it, as it is in every use: a debugger that was told `x` is told
/// `a`, not a value that is no more.
#[test]
fn replacing_a_value_replaces_it_in_the_records_too() {
    let mut parsed = module(RECORDED);
    let f = function(&mut parsed);
    let (_, value) = named(f, "x");
    let a = f.parameters()[0];
    f.replace_all_uses_with(value, Operand::Value(a));
    assert!(f.debug_records().iter().any(|one| one.what == crate::DebugWhat::Value(Operand::Value(a))), "{:?}", f.debug_records());
    assert!(!f.debug_records().iter().any(|one| one.what == crate::DebugWhat::Value(Operand::Value(value))));
}

/// An erased value has no record that names it (a record is no use, so the erase goes through): they say it is gone. An erased
/// instruction's records stand before what followed it, where the source said they were; so the verifier finds none dangling.
#[test]
fn erasing_leaves_records_gone_or_with_the_next_instruction() {
    let mut parsed = module(RECORDED);
    {
        let f = function(&mut parsed);
        let (x, value) = named(f, "x");
        let (y, _) = named(f, "y");
        let a = f.parameters()[0];
        // y reads x: x cannot go until y reads a.
        f.set_operand(y, 0, Operand::Value(a));
        assert!(f.users(value).is_empty());
        f.erase(x).expect("unused now");
        assert!(f.debug_records().iter().any(|one| one.before == y && one.what == crate::DebugWhat::Gone), "{:?}", f.debug_records());
        // The record that stood before x stands before y, which followed it.
        let before_x: Vec<u32> = f.debug_records().iter().map(|one| one.before.0).collect();
        assert!(before_x.iter().all(|&at| at != x.0), "{before_x:?}");
        let ret = f.terminator(f.entry().unwrap()).unwrap();
        f.set_operand(ret, 0, Operand::Value(a));
        f.erase(y).expect("unused now");
        assert!(f.debug_records().iter().all(|one| one.before == ret || matches!(one.what, crate::DebugWhat::Declare(_))), "{:?}", f.debug_records());
    }
    assert!(crate::verify::verify(&parsed).is_empty(), "{:?}", crate::verify::verify(&parsed));
}

/// A moved instruction leaves the records that stood before it where they were: they say what the source said at that point,
/// and the instruction going elsewhere does not move the point.
#[test]
fn moving_an_instruction_leaves_its_records_in_place() {
    let mut parsed = module(RECORDED);
    let f = function(&mut parsed);
    let (y, _) = named(f, "y");
    let (x, _) = named(f, "x");
    let ret = f.terminator(f.entry().unwrap()).unwrap();
    f.move_to(y, Position::Before(x)).expect("moves");
    // y reads x, which now follows it: the record of the value stood before y, and stays before `ret`, which followed y.
    assert!(f.debug_records().iter().all(|one| one.before != y), "{:?}", f.debug_records());
    assert!(f.debug_records().iter().any(|one| one.before == ret && matches!(one.what, crate::DebugWhat::Value(_))));
}

/// A record whose anchor is no longer in the function is the verifier's to find.
#[test]
fn the_verifier_finds_a_record_before_an_erased_instruction() {
    let mut parsed = module(RECORDED);
    let f = function(&mut parsed);
    let (x, _) = named(f, "x");
    let y = named(f, "y").0;
    let a = f.parameters()[0];
    f.set_operand(y, 0, Operand::Value(a));
    f.erase(x).expect("unused now");
    f.debug_records.push(crate::DebugRecord { before: x, variable: crate::MetadataId(0), what: crate::DebugWhat::Gone });
    let found = crate::verify::verify(&parsed);
    assert!(found.iter().any(|one| one.contains("debug record") && one.contains("no longer in the function")), "{found:?}");
}

/// A parameter the passes removed (dead-argument elimination) was named by a record, which is no use of it: the record stood naming
/// a value of no function, and printing the body (the pipeline compares bodies by their text) panicked on `hanoi` at -Os with `-g`.
#[test]
fn removing_a_parameter_leaves_the_records_that_named_it_gone() {
    let mut parsed = module("define i16 @f(i16 %a, i16 %b) {\nentry:\n  #dbg_value(i16 %b, !0)\n  ret i16 %a\n}\n\n!0 = !{!\"v\"}\n");
    let (context, f) = parsed.function_mut("f").expect("@f");
    f.remove_parameter(context, 1);
    let text = print::module(&parsed);
    assert!(text.contains("#dbg_gone(!0)") && !text.contains("#dbg_value"), "{text}");
}
