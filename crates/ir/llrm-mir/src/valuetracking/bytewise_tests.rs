//! `isBytewiseValue` and `getUnderlyingObject` against LLVM's answers on
//! `tests/vt/bytewise/*.ll` (`expected` is `tools/llvmport/vtdump.sh bw,uo` on
//! the file).

use super::bytewise::{Byte, is_bytewise_value};
use super::query::Query;
use super::underlying::{MAX_LOOKUP_SEARCH_DEPTH, get_underlying_object};
use crate::context::ConstantKind;
use crate::datalayout::DataLayout;
use crate::hash::HashMap;
use crate::module::{GlobalKind, Operand};
use crate::parse;
use crate::types::Type;

fn byte(answer: Option<Byte>) -> String {
    match answer {
        None => "none".to_owned(),
        Some(Byte::Value(_)) => "self".to_owned(),
        Some(Byte::Undef) => "undef".to_owned(),
        Some(Byte::Poison) => "poison".to_owned(),
        Some(Byte::Constant(value)) => format!("i8:{value}"),
    }
}

/// The lines `vtdump bw,uo` prints for `text`: each function's integer and
/// pointer values, then a `c` line for each constant operand that is neither
/// a global nor a block.
fn dump(text: &str) -> Vec<String> {
    let module = parse::module(text).expect("a module");
    let layout =
        module.datalayout.as_deref().map(|text| DataLayout::parse(text).expect("a layout")).unwrap_or_default();
    let mut lines = Vec::new();
    for global in &module.globals {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().unwrap_or_default();
        let query = Query::new(&module, function, &layout);
        let mut values: Vec<Operand> =
            function.parameters().iter().map(|&parameter| Operand::Value(parameter)).collect();
        for (_, inst) in function.walk() {
            values.extend(function.instruction(inst).result.map(Operand::Value));
        }
        let scalar = |value: Operand| {
            let ty = query.type_of(value).expect("a type");
            match module.context.types.get(ty) {
                Type::Pointer(_) => true,
                Type::Int(bits) => *bits <= 128,
                _ => false,
            }
        };
        let values: Vec<Operand> = values.into_iter().filter(|&value| scalar(value)).collect();
        let numbers: HashMap<Operand, usize> = values.iter().enumerate().map(|(at, &value)| (value, at)).collect();
        for (n, &value) in values.iter().enumerate() {
            let mut line = format!("{name} {n} bw={}", byte(is_bytewise_value(&query, value)));
            let ty = query.type_of(value).expect("a type");
            if matches!(module.context.types.get(ty), Type::Pointer(_)) {
                let object = get_underlying_object(&query, value, MAX_LOOKUP_SEARCH_DEPTH);
                let object = match object {
                    Operand::Constant(id) => match &module.context.get(id).kind {
                        ConstantKind::Global(global) => {
                            format!("@{}", module.globals[global.0 as usize].name.owned().unwrap_or_default())
                        }
                        ConstantKind::Null => "null".to_owned(),
                        _ => "const".to_owned(),
                    },
                    other => numbers.get(&other).map_or("?".to_owned(), ToString::to_string),
                };
                line.push_str(&format!(" uo={object}"));
            }
            lines.push(line);
        }
        let mut constants = Vec::new();
        for (_, inst) in function.walk() {
            for &operand in &function.instruction(inst).operands {
                let Operand::Constant(id) = operand else { continue };
                if matches!(module.context.get(id).kind, ConstantKind::Global(_)) {
                    continue;
                }
                constants.push(format!("{name} c bw={}", byte(is_bytewise_value(&query, operand))));
            }
        }
        lines.extend(constants);
    }
    lines
}

fn check(
    source: &str,
    expected: &str,
) {
    // The constant lines are in operand order in LLVM and in MIR's, which
    // differ for a call's callee and a phi's blocks: compared as sets.
    let sorted = |lines: Vec<String>| {
        let mut lines = lines;
        lines.sort();
        lines
    };
    let ours = sorted(dump(source));
    let theirs = sorted(expected.lines().map(str::to_owned).collect());
    assert_eq!(ours.len(), theirs.len(), "the number of lines: ours {ours:#?}\ntheirs {theirs:#?}");
    for (ours, theirs) in ours.iter().zip(&theirs) {
        assert_eq!(ours, theirs);
    }
}

#[test]
fn bytewise_values_are_as_llvm_says() {
    check(include_str!("../../tests/vt/bytewise/ints.ll"), include_str!("../../tests/vt/bytewise/ints.expected"));
}

#[test]
fn underlying_objects_are_as_llvm_says() {
    check(
        include_str!("../../tests/vt/bytewise/underlying.ll"),
        include_str!("../../tests/vt/bytewise/underlying.expected"),
    );
}

/// `isBytewiseValue` of `undef`: undef at every width, a pointer included.
#[test]
fn bytewise_undef_is_undef_as_llvm_says() {
    check(include_str!("../../tests/vt/bytewise/undef.ll"), include_str!("../../tests/vt/bytewise/undef.expected"));
}

/// `getUnderlyingObjects` looks through a select and a phi, and stops at a
/// loop-header phi that tracks a pointer loaded in the loop (`Prev = phi (a,
/// Cur)`): `Prev` and `Cur` are different objects each iteration.
#[test]
fn underlying_objects_look_through_selects_and_phis_but_not_a_trailing_loop_phi() {
    use super::underlying::get_underlying_objects;
    use crate::domtree::DomTree;
    use crate::loopinfo::LoopInfo;

    let text = "define void @f(ptr %a, ptr %b, i1 %c, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %i1, %loop ]
  %prev = phi ptr [ %a, %entry ], [ %cur, %loop ]
  %same = phi ptr [ %a, %entry ], [ %same2, %loop ]
  %slot = getelementptr ptr, ptr %b, i64 %i
  %cur = load ptr, ptr %slot
  %same2 = getelementptr i8, ptr %same, i64 1
  %pick = select i1 %c, ptr %a, ptr %b
  %i1 = add i64 %i, 1
  %done = icmp eq i64 %i1, %n
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
";
    let module = parse::module(text).expect("a module");
    let layout = DataLayout::default();
    let (_, _, function) = module.functions().next().expect("a function");
    let query = Query::new(&module, function, &layout);
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let named = |name: &str| {
        let found = function
            .parameters()
            .iter()
            .copied()
            .chain(function.walk().filter_map(|(_, inst)| function.instruction(inst).result))
            .find(|&value| function.value(value).name.as_deref() == Some(name))
            .expect(name);
        Operand::Value(found)
    };
    let objects = |name: &str| {
        let mut found = get_underlying_objects(&query, named(name), Some(&loops), MAX_LOOKUP_SEARCH_DEPTH);
        found.sort_by_key(|operand| format!("{operand:?}"));
        found
    };
    let mut want = vec![named("a"), named("b")];
    want.sort_by_key(|operand| format!("{operand:?}"));
    assert_eq!(objects("pick"), want, "a select: both arms");
    assert_eq!(objects("prev"), [named("prev")], "a phi that trails a loaded pointer stops");
    assert_eq!(objects("same"), [named("a")], "a phi that tracks the same object is looked through");
    // Without loop information every phi is looked through.
    let mut found = get_underlying_objects(&query, named("prev"), None, MAX_LOOKUP_SEARCH_DEPTH);
    found.sort_by_key(|operand| format!("{operand:?}"));
    let mut want = vec![named("a"), named("cur")];
    want.sort_by_key(|operand| format!("{operand:?}"));
    assert_eq!(found, want);
}
