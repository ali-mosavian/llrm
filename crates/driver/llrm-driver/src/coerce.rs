//! A struct of 1, 2 or 4 bytes passed by value to a register convention is one
//! scalar of its size, as Open Watcom passes it (`calling.toml`: the
//! convention's `aggregate.in_register_bytes`, and not
//! `aggregate_arguments_in_memory`). clang writes every aggregate argument
//! `byval`, in memory: the parameter becomes the integer, the callee keeps its
//! own copy and each direct call loads the value. A struct result of that size
//! is clang's `sret` pointer; the convention returns it in AL, AX or DX:AX
//! (`small_struct_results_as_scalars`).

use llrm_mir::edit::Position;
use llrm_mir::module::{GlobalKind, Module};
use llrm_mir::opcode::{Attribute, Flags, Opcode};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::{GlobalId, InstId, Operand};
use llrm_target::calling::Calling;

/// The type a parameter's `name` attribute (`byval`, `sret`) points at, with
/// its alignment.
fn typed_of(
    attrs: &[Attribute],
    name: &str,
) -> Option<(TypeId, Option<u64>)> {
    let ty = attrs
        .iter()
        .find_map(
            |one| match one {
                Attribute::Type(found, ty) if found == name => Some(*ty),
                _ => None,
            },
        )?;
    let align = attrs
        .iter()
        .find_map(
            |one| match one {
                Attribute::Int(name, value) if name == "align" => Some(*value),
                _ => None,
            },
        );
    Some((ty, align))
}

/// The `byval` type of a parameter, with its alignment.
fn byval_of(attrs: &[Attribute]) -> Option<(TypeId, Option<u64>)> {
    typed_of(attrs, "byval")
}

/// Each function of a register convention with a small `byval` parameter takes
/// the integer instead, and its direct calls pass it.
pub fn small_structs_as_scalars(
    module: &mut Module,
    calling: &Calling,
) -> Result<(), String> {
    let Some(text) = module.datalayout.clone() else { return Ok(()) };
    let layout = llrm_mir::datalayout::DataLayout::parse(&text)?;
    // (function, parameter, type, align, bytes), in parameter order.
    let mut planned: Vec<(GlobalId, usize, TypeId, Option<u64>, u64)> = vec![];
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        let Some(convention) = calling.by_number(function.calling_convention) else { continue };
        let Some(aggregate) = convention.aggregate.as_ref() else { continue };
        if convention.argument_registers.is_empty() || convention.aggregate_arguments_in_memory {
            continue;
        }
        for (index, attrs) in function.parameter_attrs.iter().enumerate() {
            let Some((ty, align)) = byval_of(attrs) else { continue };
            let bytes = layout.alloc_size(&module.context.types, ty);
            if aggregate.in_register_bytes.contains(&(bytes as i64)) {
                planned.push((GlobalId(at as u32), index, ty, align, bytes));
            }
        }
    }
    // Later parameters first: an earlier index stays where it is.
    for &(id, index, ty, align, bytes) in planned.iter().rev() {
        let scalar = module.context.types.int(bytes as u32 * 8);
        let Module { context, globals, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
        let made = function.insert_parameters(context, index, &[scalar])[0];
        if let Some(entry) = function.entry() {
            let old = function.parameters()[index + 1];
            let first = function.block(entry).first().expect("a block with instructions");
            let pointer = context.types.ptr(0);
            let slot = function.create_instruction(
                Opcode::Alloca { allocated: ty, align, address_space: 0 },
                pointer,
                Vec::new(),
                Flags::default(),
                Some("byval"),
            );
            function.insert(slot, Position::Before(first)).map_err(|e| e.to_string())?;
            let address = Operand::Value(function.instruction(slot).result.expect("an address"));
            let store = function.create_instruction(
                Opcode::Store { align: None, volatile: false },
                context.types.void(),
                vec![Operand::Value(made), address],
                Flags::default(),
                None,
            );
            function.insert(store, Position::Before(first)).map_err(|e| e.to_string())?;
            function.replace_all_uses_with(old, address);
        }
        function.remove_parameter(context, index + 1);
        let ty_now = function.ty;
        // Direct calls, in every function.
        for caller in 0..globals.len() {
            let GlobalKind::Function(caller_fn) = &mut globals[caller].kind else { continue };
            let calls: Vec<InstId> = caller_fn
                .walk()
                .filter_map(|(_, inst)| {
                    let instruction = caller_fn.instruction(inst);
                    let Opcode::Call(_) = instruction.opcode else { return None };
                    let Some(Operand::Constant(callee)) = instruction.operands.last().copied() else { return None };
                    matches!(
                        context.get(callee).kind,
                        llrm_mir::ConstantKind::Global(g) if g == id
                    )
                    .then_some(inst)
                })
                .collect();
            for call in calls {
                let pointer = caller_fn.instruction(call).operands[index];
                let load = caller_fn.create_instruction(
                    Opcode::Load { align: None, volatile: false },
                    scalar,
                    vec![pointer],
                    Flags::default(),
                    None,
                );
                caller_fn.insert(load, Position::Before(call)).map_err(|e| e.to_string())?;
                let loaded = Operand::Value(caller_fn.instruction(load).result.expect("a value"));
                caller_fn.replace_argument(call, index, &[loaded], ty_now);
            }
        }
        // The declaration's own attribute is gone with the parameter.
    }
    Ok(())
}

/// Each function of a convention that returns a struct of 1, 2 or 4 bytes in
/// registers (`aggregate.in_register_bytes`: Borland's AL, AX, DX:AX) and that
/// clang gave an `sret` pointer for it returns the integer instead, and its
/// calls, direct or through a pointer (the call carries its convention), store
/// the result where the pointer pointed.
pub fn small_struct_results_as_scalars(
    module: &mut Module,
    calling: &Calling,
) -> Result<(), String> {
    let Some(text) = module.datalayout.clone() else { return Ok(()) };
    let layout = llrm_mir::datalayout::DataLayout::parse(&text)?;
    // (function, parameter, type, align, bytes).
    let mut planned: Vec<(GlobalId, usize, TypeId, Option<u64>, u64)> = vec![];
    for (at, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        let Some(convention) = calling.by_number(function.calling_convention) else { continue };
        let Some(aggregate) = convention.aggregate.as_ref() else { continue };
        let Some((index, (ty, align))) = function
            .parameter_attrs
            .iter()
            .enumerate()
            .find_map(|(index, attrs)| typed_of(attrs, "sret").map(|found| (index, found)))
        else {
            continue;
        };
        let bytes = layout.alloc_size(&module.context.types, ty);
        if aggregate.in_register_bytes.contains(&(bytes as i64)) {
            planned.push((GlobalId(at as u32), index, ty, align, bytes));
        }
    }
    for &(id, index, ty, align, bytes) in &planned {
        let scalar = module.context.types.int(bytes as u32 * 8);
        let Module { context, globals, .. } = &mut *module;
        let GlobalKind::Function(function) = &mut globals[id.0 as usize].kind else { unreachable!("a function") };
        if let Some(entry) = function.entry() {
            let old = function.parameters()[index];
            let first = function.block(entry).first().expect("a block with instructions");
            let pointer = context.types.ptr(0);
            let slot = function.create_instruction(
                Opcode::Alloca { allocated: ty, align, address_space: 0 },
                pointer,
                Vec::new(),
                Flags::default(),
                Some("sret"),
            );
            function.insert(slot, Position::Before(first)).map_err(|e| e.to_string())?;
            let address = Operand::Value(function.instruction(slot).result.expect("an address"));
            function.replace_all_uses_with(old, address);
            // The value the struct holds at each return.
            let returns: Vec<InstId> =
                function.walk().map(|(_, inst)| inst).filter(|&inst| function.instruction(inst).opcode == Opcode::Ret).collect();
            for ret in returns {
                let load = function.create_instruction(
                    Opcode::Load { align: None, volatile: false },
                    scalar,
                    vec![address],
                    Flags::default(),
                    None,
                );
                function.insert(load, Position::Before(ret)).map_err(|e| e.to_string())?;
                let loaded = Operand::Value(function.instruction(load).result.expect("a value"));
                function.set_operands(ret, vec![loaded]);
            }
        }
        function.remove_parameter(context, index);
        function.set_return_type(context, scalar);
    }
    // Every call, direct or through a pointer, whose own convention returns
    // that size in registers and that passes an `sret` argument.
    let Module { context, globals, .. } = &mut *module;
    for caller in 0..globals.len() {
        let GlobalKind::Function(caller_fn) = &mut globals[caller].kind else { continue };
        let calls: Vec<(InstId, usize, TypeId, u64)> = caller_fn
            .walk()
            .filter_map(|(_, inst)| {
                let Opcode::Call(info) = &caller_fn.instruction(inst).opcode else { return None };
                let aggregate = calling.by_number(info.calling_convention)?.aggregate.as_ref()?;
                let (index, (ty, _)) =
                    info.argument_attrs.iter().enumerate().find_map(|(index, attrs)| typed_of(attrs, "sret").map(|found| (index, found)))?;
                let bytes = layout.alloc_size(&context.types, ty);
                aggregate.in_register_bytes.contains(&(bytes as i64)).then_some((inst, index, info.function_type, bytes))
            })
            .collect();
        for (call, index, old_type, bytes) in calls {
            let scalar = context.types.int(bytes as u32 * 8);
            let Type::Function { mut parameters, variadic, .. } = context.types.get(old_type).clone() else {
                return Err("a call whose type is not a function type".into());
            };
            parameters.remove(index);
            let new_type = context.types.intern(Type::Function { returns: scalar, parameters, variadic });
            let pointer = caller_fn.instruction(call).operands[index];
            caller_fn.replace_argument(call, index, &[], new_type);
            let result = caller_fn.give_call_result(call, scalar, new_type);
            let store = caller_fn.create_instruction(
                Opcode::Store { align: None, volatile: false },
                context.types.void(),
                vec![Operand::Value(result), pointer],
                Flags::default(),
                None,
            );
            caller_fn.insert(store, Position::After(call)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
