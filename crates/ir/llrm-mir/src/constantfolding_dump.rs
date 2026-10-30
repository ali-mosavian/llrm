//! The text `tools/llvmport/constfold_dump.cpp` prints, for
//! `constant_fold_instruction`: `<function> <n> <opcode> <constant | ->` for
//! every instruction with a result whose operands are all constants, and the
//! constant as that driver writes it. `examples/cfdump.rs` prints it;
//! `constantfolding_tests` compares it with LLVM's.

use crate::constantfolding::{Env, constant_fold_instruction, is_poison};
use crate::constfold::is_null_value;
use crate::datalayout::DataLayout;
use crate::module::Operand;
use crate::opcode::{CAST, Opcode, spelling};
use crate::parse;
use crate::{ConstantExpr, ConstantId, ConstantKind, Context, FloatKind, GlobalKind, GlobalValue, Type};

/// The lines for the module `text`, one per instruction.
pub fn dump(text: &str) -> Result<String, String> {
    let mut module = parse::module(text).map_err(|why| why.to_string())?;
    let layout = match module.datalayout.as_deref() {
        Some(text) => DataLayout::parse(text)?,
        None => DataLayout::default(),
    };
    let mut out = String::new();
    for global in &module.globals {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let name = global.name.owned().unwrap_or_default();
        let mut n = 0;
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            if instruction.result.is_none() {
                continue;
            }
            let operands: Vec<&Operand> = if instruction.opcode == Opcode::Phi {
                instruction.operands.iter().step_by(2).collect()
            } else {
                instruction.operands.iter().collect()
            };
            if !operands.iter().all(|one| matches!(one, Operand::Constant(_))) {
                continue;
            }
            let env = Env { globals: &module.globals, layout: &layout };
            let folded = constant_fold_instruction(&mut module.context, env, function, inst);
            let shown = folded.map_or("-".to_owned(), |c| show(&module.context, &module.globals, c));
            out.push_str(&format!("{name} {n} {} {shown}\n", instruction.opcode.mnemonic()));
            n += 1;
        }
    }
    Ok(out)
}

fn show(
    context: &Context,
    globals: &[GlobalValue],
    c: ConstantId,
) -> String {
    let constant = context.get(c);
    let t = context.types.display(constant.ty);
    let members = |ids: &[ConstantId]| ids.iter().map(|&one| show(context, globals, one)).collect::<Vec<_>>();
    match &constant.kind {
        ConstantKind::Int(bits) => format!("{t} {bits}"),
        ConstantKind::Float(bits) => match context.types.get(constant.ty) {
            Type::Float(FloatKind::Float) => format!("{t} 0x{:X}", bits & 0xFFFF_FFFF),
            Type::Float(FloatKind::Double) => format!("{t} 0x{bits:X}"),
            _ => format!("{t} fp"),
        },
        ConstantKind::Float80(bits) => format!("{t} 0xK{bits:020X}"),
        ConstantKind::Null if context.types.int_bits(constant.ty).is_some() => format!("{t} 0"),
        ConstantKind::Null => format!("{t} null"),
        ConstantKind::Poison => format!("{t} poison"),
        ConstantKind::Undef => format!("{t} undef"),
        ConstantKind::Zero if context.types.int_bits(constant.ty).is_some() => format!("{t} 0"),
        ConstantKind::Zero if matches!(context.types.get(constant.ty), Type::Float(_)) => format!("{t} 0x0"),
        ConstantKind::Zero if matches!(context.types.get(constant.ty), Type::Pointer(_)) => format!("{t} null"),
        ConstantKind::Zero => format!("{t} zeroinitializer"),
        ConstantKind::Global(global) => {
            format!("{t} @{}", globals[global.0 as usize].name.owned().unwrap_or_default())
        }
        ConstantKind::Bytes(bytes) => {
            if is_null_value(context, c) {
                return format!("{t} zeroinitializer");
            }
            format!("{t} [{}]", bytes.iter().map(|one| format!("i8 {one}")).collect::<Vec<_>>().join(", "))
        }
        ConstantKind::Aggregate(ids) => {
            if is_poison(context, c) {
                return format!("{t} poison");
            }
            if is_null_value(context, c) {
                return format!("{t} zeroinitializer");
            }
            let body = members(ids).join(", ");
            match context.types.get(constant.ty) {
                Type::Array { .. } => format!("{t} [{body}]"),
                Type::Vector { .. } => format!("{t} <{body}>"),
                _ => format!("{t} {{ {body} }}"),
            }
        }
        ConstantKind::Expr(ConstantExpr::GetElementPtr { source, inbounds, nusw, nuw, operands }) => {
            let flags = crate::print::gep_flag_words(*inbounds, *nusw, *nuw);
            let flags = flags.trim_end().replace(' ', ",");
            format!("{t} gep[{flags}] {} ({})", context.types.display(*source), members(operands).join(", "))
        }
        ConstantKind::Expr(ConstantExpr::Cast { op, value }) => {
            format!("{t} {} ({})", spelling(&CAST, *op), show(context, globals, *value))
        }
        ConstantKind::Expr(ConstantExpr::Binary { op, lhs, rhs }) => {
            format!(
                "{t} {} ({}, {})",
                spelling(&crate::opcode::BINARY, *op),
                show(context, globals, *lhs),
                show(context, globals, *rhs)
            )
        }
    }
}
