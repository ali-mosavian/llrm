//! `isBytewiseValue` and `getUnderlyingObject` against LLVM's answers on
//! `tests/vt/bytewise/*.ll` (`expected` is `tools/llvmport/vtdump.sh bw,uo` on
//! the file).

use crate::hash::HashMap;

use super::bytewise::{Byte, is_bytewise_value};
use super::query::Query;
use super::underlying::{MAX_LOOKUP_SEARCH_DEPTH, get_underlying_object};
use crate::context::ConstantKind;
use crate::datalayout::DataLayout;
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
        let name = global.name.clone().unwrap_or_default();
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
                            format!("@{}", module.globals[global.0 as usize].name.clone().unwrap_or_default())
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
