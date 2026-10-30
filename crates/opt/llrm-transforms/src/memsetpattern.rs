//! Expands `llvm.experimental.memset.pattern` into a store loop in MIR before
//! instruction selection, as the pin's `PreISelIntrinsicLowering` does (the
//! DAG never sees the intrinsic).
//!
//! # How it works
//! The block is split after the call. A constant length of zero makes
//! nothing; a constant one jumps to the loop, a variable one first tests
//! `len != 0`. The loop body stores the pattern at `dst[i]`, steps `i` and
//! repeats while `i + 1 < len`. One element per iteration: x86 asks for no
//! wider loop type (`getMemcpyLoopLoweringType` is `i8`).
//!
//! # Example
//! `call @llvm.experimental.memset.pattern(ptr %p, i16 1824, i32 %n, i1 false)`
//!
//! ```text
//!   %c = icmp ne i32 %n, 0 ; br %c, main-body, post-expansion
//! main-body:  %i = phi [0, pre], [%j, main-body]
//!             %a = getelementptr inbounds i16, ptr %p, %i ; store i16 1824, %a
//!             %j = add %i, 1 ; %k = icmp ult %j, %n ; br %k, main-body, post
//! ```
//!
//! # Runs
//! Once per module in `driver::optimized`, for `--isel=dag`: the old selector
//! lowers the intrinsic itself.
//!
//! # See also
//! LLVM's `createMemSetPatternLoop`
//! ([LowerMemIntrinsics.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Utils/LowerMemIntrinsics.cpp#L1304)).

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Module, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, Flags, IntPredicate, Opcode};

use crate::edges::split_block_at;
use crate::expansion;

/// Every `memset.pattern` call of `module` as a loop.
pub fn expanded(module: &mut Module) -> Result<(), String> {
    let layout = module.datalayout.as_deref().map_or_else(|| Ok(DataLayout::default()), DataLayout::parse)?;
    expansion::expand_calls(
        module,
        |intrinsic| matches!(intrinsic, Intrinsic::MemSetPattern).then_some(()),
        |context, function, call, ()| {
            let instruction = function.instruction(call).clone();
            let (dst, value, len, volatile) =
                (instruction.operands[0], instruction.operands[1], instruction.operands[2], instruction.operands[3]);
            let int = |context: &llrm_mir::context::Context, operand: Operand| {
                function.operand_type(context, operand).ok_or("a typed operand")
            };
            let (pointer_type, element, length_type) = (int(context, dst)?, int(context, value)?, int(context, len)?);
            let zero = context.constant(Constant { ty: length_type, kind: ConstantKind::Int(0) });
            let one = context.constant(Constant { ty: length_type, kind: ConstantKind::Int(1) });
            let constant_len = match len {
                Operand::Constant(id) => match context.get(id).kind {
                    ConstantKind::Int(bits) => Some(bits),
                    ConstantKind::Zero | ConstantKind::Null => Some(0),
                    _ => None,
                },
                _ => None,
            };
            if constant_len == Some(0) {
                function.erase(call).map_err(|why| why.to_string())?;
                return Ok(());
            }
            let volatile = matches!(
                volatile,
                Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(1))
            );
            let dst_align = match &instruction.opcode {
                Opcode::Call(info) => info
                    .argument_attrs
                    .first()
                    .into_iter()
                    .flatten()
                    .find_map(|attr| match attr {
                        Attribute::Int(name, value) if name == "align" => Some(*value),
                        _ => None,
                    })
                    .unwrap_or(1),
                _ => 1,
            };
            let element_size = layout.alloc_size(&context.types, element);
            let align = dst_align.min(1 << element_size.trailing_zeros());
            let pre = function.parent(call).ok_or("a placed call")?;
            let at = function.next_in_block(call).ok_or("a terminated block")?;
            let post = split_block_at(function, pre, at);
            function.set_block_name(post, "memset.pattern-post-expansion");
            let main = function.create_block(Some("memset.pattern-expansion-main-body"));
            function.insert_block(main, Some(pre)).map_err(|why| why.to_string())?;
            let jump = function.terminator(pre).ok_or("a terminated block")?;
            let void = function.instruction(jump).ty;
            let bool_type = context.types.int(1);
            function.erase(jump).map_err(|why| why.to_string())?;
            let entry = if constant_len.is_some() {
                function.create_instruction(Opcode::Br, void, vec![Operand::Block(main)], Flags::default(), None)
            } else {
                let test = function.create_instruction(
                    Opcode::ICmp(IntPredicate::Ne),
                    bool_type,
                    vec![len, Operand::Constant(zero)],
                    Flags::default(),
                    None,
                );
                function.insert(test, Position::End(pre)).map_err(|why| why.to_string())?;
                let test = Operand::Value(function.instruction(test).result.ok_or("a compare result")?);
                function.create_instruction(
                    Opcode::Br,
                    void,
                    vec![test, Operand::Block(main), Operand::Block(post)],
                    Flags::default(),
                    None,
                )
            };
            function.insert(entry, Position::End(pre)).map_err(|why| why.to_string())?;
            let phi =
                function.create_instruction(Opcode::Phi, length_type, Vec::new(), Flags::default(), Some("loop-index"));
            let index = Operand::Value(function.instruction(phi).result.ok_or("a phi result")?);
            let address = function.create_instruction(
                Opcode::GetElementPtr { source: element },
                pointer_type,
                vec![dst, index],
                Flags::INBOUNDS,
                None,
            );
            let store = function.create_instruction(
                Opcode::Store { align: Some(align), volatile },
                void,
                vec![value, Operand::Value(function.instruction(address).result.ok_or("an address")?)],
                Flags::default(),
                None,
            );
            let next = function.create_instruction(
                Opcode::Binary(BinaryOp::Add),
                length_type,
                vec![index, Operand::Constant(one)],
                Flags::default(),
                None,
            );
            let next_value = Operand::Value(function.instruction(next).result.ok_or("a sum")?);
            let again = function.create_instruction(
                Opcode::ICmp(IntPredicate::Ult),
                bool_type,
                vec![next_value, len],
                Flags::default(),
                None,
            );
            let again_value = Operand::Value(function.instruction(again).result.ok_or("a compare result")?);
            let back = function.create_instruction(
                Opcode::Br,
                void,
                vec![again_value, Operand::Block(main), Operand::Block(post)],
                Flags::default(),
                None,
            );
            for inst in [phi, address, store, next, again, back] {
                function.insert(inst, Position::End(main)).map_err(|why| why.to_string())?;
            }
            function.set_operands(
                phi,
                vec![Operand::Constant(zero), Operand::Block(pre), next_value, Operand::Block(main)],
            );
            function.erase(call).map_err(|why| why.to_string())?;
            Ok(())
        },
    )
}
