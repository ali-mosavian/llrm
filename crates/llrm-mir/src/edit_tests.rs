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
