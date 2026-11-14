//! Turns the `llvm.memcpy` and `llvm.memset` calls of chosen functions into
//! byte loops, before instruction selection.
//!
//! # How it works
//! The pin's `PreISelIntrinsicLowering` expands a memory intrinsic into a loop
//! when the target has no routine for it (`createMemCpyLoopUnknownSize`). Here
//! the target picks the functions: it prices the function both ways, call and
//! loop, from its own backend's output, and expands the ones where the loop is
//! cheaper. A volatile copy stays a call. The rewrite is linear in the code it
//! adds.
//!
//! DEVIATE: the loop steps both pointers and counts the length down, where the
//! pin's steps an index and leaves the pointers to loop strength reduction,
//! which has run by then. Exit: none.
//!
//! DEVIATE: one choice for all the copies of a function, where the pin's is per
//! call; pricing each call apart is a compile of the function for each. Exit:
//! the allocator pricing a call by what it spills.
//!
//! # Example
//! `call @llvm.memcpy(ptr %d, ptr %s, i16 %n, i1 false)`:
//!
//! ```text
//!   %z = icmp ne i16 %n, 0 ; br %z, loop, post
//! loop: %s1 = phi [%s, pre], [%s2, loop]   %d1 = phi [%d, pre], [%d2, loop]
//!       %k = phi [%n, pre], [%k2, loop]
//!       %v = load i8, ptr %s1 ; store i8 %v, ptr %d1
//!       %s2 = gep inbounds i8, %s1, 1 ; %d2 = gep inbounds i8, %d1, 1
//!       %k2 = sub %k, 1 ; %c = icmp ne %k2, 0 ; br %c, loop, post
//! ```
//!
//! # Runs
//! In a target's driver, on a copy of the module, for the functions it names.
//!
//! # See also
//! LLVM's `createMemCpyLoopUnknownSize` and `createMemSetLoop`
//! ([LowerMemIntrinsics.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/LowerMemIntrinsics.cpp#L35)).

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, Function, GlobalKind, GlobalValue, InstId, Module, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::TypeId;

use crate::edges::split_block_at;

/// How many loops each block of `function` is in, by block number.
pub fn loop_depths(function: &Function) -> Vec<u32> {
    let shape = llrm_analysis::cfg::Shape::of(function);
    let mut depths = vec![0; function.block_capacity()];
    for loop_ in &shape.loops {
        for &block in loop_.body.iter() {
            depths[block as usize] += 1;
        }
    }
    depths
}

/// The non-volatile `llvm.memcpy` calls of `function` of a length known only at
/// run time: a constant one is the selector's, inline moves up to its limit.
fn copies(
    context: &Context,
    intrinsics: &[Option<Intrinsic>],
    function: &Function,
) -> IdSet<InstId> {
    function
        .walk()
        .filter_map(|(_, inst)| {
            let callee = llrm_mir::memory::callee(context, function, inst)?;
            let copy = matches!(intrinsics[callee.0 as usize]?, Intrinsic::MemCpy | Intrinsic::MemSet);
            (copy
                && constant_value(context, function.instruction(inst).operands[2]).is_none()
                && !volatile(context, function, inst))
            .then_some(inst)
        })
        .collect()
}

/// How many functions of `module` call the library routine `routine`
/// (`memcpy`, `memset`) with a length known only at run time: the ones that
/// share its bytes in the linked image. Any other routine counts one.
pub fn callers(
    module: &Module,
    routine: &str,
) -> usize {
    let wanted = match routine {
        "memcpy" => Intrinsic::MemCpy,
        "memset" => Intrinsic::MemSet,
        _ => return 1,
    };
    let intrinsics: Vec<Option<Intrinsic>> = module.globals.iter().map(GlobalValue::intrinsic).collect();
    module
        .globals
        .iter()
        .filter(|global| {
            let GlobalKind::Function(function) = &global.kind else { return false };
            copies(&module.context, &intrinsics, function).iter().any(|call| {
                llrm_mir::memory::callee(&module.context, function, call)
                    .is_some_and(|callee| intrinsics[callee.0 as usize] == Some(wanted))
            })
        })
        .count()
}

/// The functions of `module` that call `llvm.memcpy` or `llvm.memset` and
/// could have it as a loop.
pub fn candidates(module: &Module) -> Vec<String> {
    let intrinsics: Vec<Option<Intrinsic>> = module.globals.iter().map(GlobalValue::intrinsic).collect();
    module
        .globals
        .iter()
        .filter_map(|global| {
            let GlobalKind::Function(function) = &global.kind else { return None };
            let name = global.name.as_deref()?;
            (!copies(&module.context, &intrinsics, function).is_empty()).then(|| name.to_owned())
        })
        .collect()
}

/// Every non-volatile `llvm.memcpy` in the functions of `module` named by
/// `wanted`, as a loop.
pub fn expanded(
    module: &mut Module,
    wanted: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    let intrinsics: Vec<Option<Intrinsic>> = module.globals.iter().map(GlobalValue::intrinsic).collect();
    let context = &mut module.context;
    for global in &mut module.globals {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        if !global.name.as_deref().is_some_and(wanted) {
            continue;
        }
        for call in copies(context, &intrinsics, function).iter() {
            let set = llrm_mir::memory::callee(context, function, call)
                .is_some_and(|callee| matches!(intrinsics[callee.0 as usize], Some(Intrinsic::MemSet)));
            loop_of(context, function, call, set)?;
        }
    }
    Ok(())
}

fn volatile(
    context: &Context,
    function: &Function,
    call: InstId,
) -> bool {
    constant_value(context, function.instruction(call).operands[3]).is_some_and(|flag| flag != 0)
}

fn constant_value(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            ConstantKind::Zero | ConstantKind::Null => Some(0),
            _ => None,
        },
        _ => None,
    }
}

/// The alignment `call` states for its `argument`th operand.
fn argument_align(
    function: &Function,
    call: InstId,
    argument: usize,
) -> u64 {
    match &function.instruction(call).opcode {
        Opcode::Call(info) => info
            .argument_attrs
            .get(argument)
            .into_iter()
            .flatten()
            .find_map(|attr| match attr {
                Attribute::Int(name, value) if name == "align" => Some(*value),
                _ => None,
            })
            .unwrap_or(1),
        _ => 1,
    }
}

/// Appends an instruction to `block`: it and its result, if it has one.
fn append(
    function: &mut Function,
    block: BlockId,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
    flags: Flags,
) -> Result<(InstId, Option<Operand>), String> {
    let made = function.create_instruction(opcode, ty, operands, flags, None);
    function.insert(made, Position::End(block)).map_err(|why| why.to_string())?;
    Ok((made, function.instruction(made).result.map(Operand::Value)))
}

/// `call` replaced by the byte loop.
fn loop_of(
    context: &mut Context,
    function: &mut Function,
    call: InstId,
    set: bool,
) -> Result<(), String> {
    let instruction = function.instruction(call).clone();
    // a copy: (dst, src, len, volatile); a set: (dst, byte, len, volatile)
    let (dst, src, len) = (instruction.operands[0], instruction.operands[1], instruction.operands[2]);
    let (pointer_type, source_type, length_type) = (
        function.operand_type(context, dst).ok_or("a typed operand")?,
        function.operand_type(context, src).ok_or("a typed operand")?,
        function.operand_type(context, len).ok_or("a typed operand")?,
    );
    let name = if set { "memset" } else { "memcpy" };
    let byte = context.types.int(8);
    let bool_type = context.types.int(1);
    let known = constant_value(context, len);
    let (dst_align, src_align) = (argument_align(function, call, 0), argument_align(function, call, 1));
    if known == Some(0) {
        return function.erase(call).map_err(|why| why.to_string());
    }
    let int = |context: &mut Context, value: u128| {
        Operand::Constant(context.constant(Constant { ty: length_type, kind: ConstantKind::Int(value) }))
    };
    let (zero, one) = (int(context, 0), int(context, 1));
    let pre = function.parent(call).ok_or("a placed call")?;
    let at = function.next_in_block(call).ok_or("a terminated block")?;
    let post = split_block_at(function, pre, at);
    function.set_block_name(post, &format!("{name}-post-expansion"));
    let main = function.create_block(Some(&format!("{name}-expansion-main-body")));
    function.insert_block(main, Some(pre)).map_err(|why| why.to_string())?;
    let jump = function.terminator(pre).ok_or("a terminated block")?;
    let void = function.instruction(jump).ty;
    function.erase(jump).map_err(|why| why.to_string())?;
    if known.is_some() {
        append(function, pre, Opcode::Br, void, vec![Operand::Block(main)], Flags::default())?;
    } else {
        let (_, test) =
            append(function, pre, Opcode::ICmp(IntPredicate::Ne), bool_type, vec![len, zero], Flags::default())?;
        let test = test.ok_or("a compare result")?;
        let branch = vec![test, Operand::Block(main), Operand::Block(post)];
        append(function, pre, Opcode::Br, void, branch, Flags::default())?;
    }
    let result = |made: (InstId, Option<Operand>)| (made.0, made.1.expect("a result"));
    // a copy walks the source too; a set stores its byte
    let from = (!set)
        .then(|| append(function, main, Opcode::Phi, source_type, Vec::new(), Flags::default()))
        .transpose()?
        .map(result);
    let (to, to_value) = result(append(function, main, Opcode::Phi, pointer_type, Vec::new(), Flags::default())?);
    let (left, left_value) = result(append(function, main, Opcode::Phi, length_type, Vec::new(), Flags::default())?);
    let stored = match from {
        Some((_, from_value)) => {
            let (_, loaded) = result(append(
                function,
                main,
                Opcode::Load { align: Some(src_align), volatile: false },
                byte,
                vec![from_value],
                Flags::default(),
            )?);
            loaded
        }
        None => src,
    };
    append(
        function,
        main,
        Opcode::Store { align: Some(dst_align), volatile: false },
        void,
        vec![stored, to_value],
        Flags::default(),
    )?;
    let gep = Opcode::GetElementPtr { source: byte };
    let next_from = match from {
        Some((_, from_value)) => {
            Some(result(append(function, main, gep.clone(), source_type, vec![from_value, one], Flags::INBOUNDS)?).1)
        }
        None => None,
    };
    let (_, next_to) = result(append(function, main, gep, pointer_type, vec![to_value, one], Flags::INBOUNDS)?);
    let (_, next_left) = result(append(
        function,
        main,
        Opcode::Binary(BinaryOp::Sub),
        length_type,
        vec![left_value, one],
        Flags::default(),
    )?);
    let (_, again) = result(append(
        function,
        main,
        Opcode::ICmp(IntPredicate::Ne),
        bool_type,
        vec![next_left, zero],
        Flags::default(),
    )?);
    let back = vec![again, Operand::Block(main), Operand::Block(post)];
    append(function, main, Opcode::Br, void, back, Flags::default())?;
    if let (Some((from, _)), Some(next_from)) = (from, next_from) {
        function.set_operands(from, vec![src, Operand::Block(pre), next_from, Operand::Block(main)]);
    }
    function.set_operands(to, vec![dst, Operand::Block(pre), next_to, Operand::Block(main)]);
    function.set_operands(left, vec![len, Operand::Block(pre), next_left, Operand::Block(main)]);
    function.erase(call).map_err(|why| why.to_string())
}
