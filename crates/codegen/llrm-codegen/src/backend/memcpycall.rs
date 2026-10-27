//! Turns an `llvm.memcpy` the target should not copy inline into a call of
//! `memcpy`, as the pin's `EmitTargetCodeForMemcpy` leaves it: a call for a
//! length the target cannot move in a few loads and stores.
//! Translated from llvm/lib/Target/X86/X86SelectionDAGInfo.cpp at d1106deb71cc.
//!
//! # How it works
//! A copy of a variable length, of a constant length over [`INLINE_LIMIT`]
//! (`getMaxInlineSizeThreshold`) or of pointers aligned under four bytes is
//! a call of `memcpy(dst, src, len)` in the ABI's default convention; the
//! rest stays for the selector. Only on a target whose runtime says it has
//! `memcpy` (`libcalls` of its `c.toml`): the selector's `rep movs` stays
//! for the others (DEVIATE I1, AM-F-1).
//!
//! # Example
//! `call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)` becomes
//! `call ptr @memcpy(ptr %d, ptr %s, i32 %n)`; with a constant length of 16
//! and 4-byte alignment it is left alone.
//!
//! # Runs
//! Once per module in `driver::optimized`, when the target names `memcpy`
//! among its libcalls.
//!
//! # See also
//! LLVM's `X86SelectionDAGInfo::EmitTargetCodeForMemcpy`
//! ([#L328](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86SelectionDAGInfo.cpp#L328)).

use llrm_mir::context::ConstantKind;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{Linkage, Module, Operand};
use llrm_mir::opcode::{Attribute, CallInfo, CastOp, Flags, Opcode};

use super::expansion;

/// `X86SelectionDAGInfo::getMaxInlineSizeThreshold`: a constant copy over this
/// many bytes is a call.
pub const INLINE_LIMIT: u128 = 128;

/// The bytes of a constant memset's `rep stosb` through es:di, as
/// `memset` makes it tuned for size: `lea di`, ES saved and set, the byte,
/// the count, the fill, and DI's save in the frame.
pub(crate) const FILL_BYTES: i64 = 3 + 4 + 2 + 3 + 2 + 2;

/// The bytes of the stores a memset expands a constant `length` to, each at a
/// one-byte displacement: two or more dwords store a register.
pub(crate) fn stored_bytes(length: i64) -> i64 {
    let dwords = length / 4;
    let each = if dwords > 1 { 4 } else { 8 };
    let setup = if dwords > 1 { 6 } else { 0 };
    setup + dwords * each + if length % 4 >= 2 { 5 } else { 0 } + if length % 2 == 1 { 4 } else { 0 }
}

/// `findOptimalMemOpLowering` for x86 without SSE: the width in bytes of each
/// memory operation covering `size` bytes, the widest being `i32`; `None` past
/// `limit` operations. A last operation wider than the bytes left overlaps the
/// one before it.
pub(crate) fn inline_widths(
    size: u64,
    widest: u64,
    limit: usize,
    volatile: bool,
) -> Option<Vec<u64>> {
    let (mut left, mut width) = (size, widest);
    let mut ops = Vec::new();
    while left > 0 {
        let mut taken = width;
        while taken > left {
            let narrower = width / 2;
            if !ops.is_empty() && !volatile && narrower < left {
                // a misaligned access is fast on x86: overlap the previous
                // bytes
                taken = left;
            } else {
                width = narrower;
                taken = narrower;
            }
        }
        if ops.len() + 1 > limit {
            return None;
        }
        ops.push(width);
        left -= taken;
    }
    Some(ops)
}

/// Every `llvm.memcpy` of `module` the target copies by a call, as a call of
/// the routine `symbol` in calling convention `convention` (LLVM's number).
pub fn expanded(
    module: &mut Module,
    convention: u32,
    symbol: &str,
) -> Result<(), String> {
    let needs = |intrinsic: Intrinsic| matches!(intrinsic, Intrinsic::MemCpy).then_some(());
    let mut any = false;
    expansion::expand_calls(module, needs, |context, function, call, ()| {
        any |= calls(context, function, call);
        Ok(())
    })?;
    if !any {
        return Ok(());
    }
    let (pointer, word, returns) = {
        let types = &module.context.types;
        (types.ptr(0), types.int(32), types.ptr(0))
    };
    let ty = module
        .context
        .types
        .intern(
            llrm_mir::types::Type::Function {
                returns,
                parameters: vec![pointer, pointer, word],
                variadic: false,
            },
        );
    let id = match module.named(symbol) {
        Some(id) => id,
        None => module.add_function(symbol, ty, Linkage::External)?,
    };
    let reference = module.reference(id);
    expansion::expand_calls(module, needs, |context, function, call, ()| {
        if !calls(context, function, call) {
            return Ok(());
        }
        let instruction = function.instruction(call).clone();
        let (dst, src, len) = (instruction.operands[0], instruction.operands[1], instruction.operands[2]);
        let bits = function
            .operand_type(context, len)
            .and_then(|ty| context.types.int_bits(ty))
            .ok_or("a copy of an integer length")?;
        let len = match bits.cmp(&32) {
            std::cmp::Ordering::Equal => len,
            order => {
                let op = if order.is_lt() { CastOp::ZExt } else { CastOp::Trunc };
                let made = function.create_instruction(Opcode::Cast(op), word, vec![len], Flags::default(), None);
                function.insert(made, Position::Before(call)).map_err(|why| why.to_string())?;
                Operand::Value(function.instruction(made).result.expect("a value"))
            }
        };
        let info = CallInfo {
            function_type: ty,
            calling_convention: convention,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); 3],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        let made = function.create_instruction(
            Opcode::Call(Box::new(info)),
            pointer,
            vec![dst, src, len, Operand::Constant(reference)],
            Flags::default(),
            None,
        );
        function.insert(made, Position::Before(call)).map_err(|why| why.to_string())?;
        function.erase(call).map_err(|why| why.to_string())
    })
}

/// Whether `call` is a copy the target does not make inline.
fn calls(
    context: &llrm_mir::context::Context,
    function: &llrm_mir::module::Function,
    call: llrm_mir::module::InstId,
) -> bool {
    let instruction = function.instruction(call);
    let Opcode::Call(info) = &instruction.opcode else { return false };
    let align = |at: usize| {
        info.argument_attrs
            .get(at)
            .into_iter()
            .flatten()
            .find_map(|attr| match attr {
                Attribute::Int(name, value) if name == "align" => Some(*value),
                _ => None,
            })
            .unwrap_or(1)
    };
    // `SelectionDAG::getMemcpy` copies by loads and stores first, whatever the
    // alignment (x86 allows misaligned accesses); the target's code and the
    // call come only after that fails.
    if let Operand::Constant(id) = instruction.operands[2]
        && let ConstantKind::Int(bytes) = context.get(id).kind
    {
        let volatile = matches!(
            instruction.operands.get(3),
            Some(Operand::Constant(flag)) if matches!(context.get(*flag).kind, ConstantKind::Int(1))
        );
        let limit =
            if llrm_mir::memory::has(&function.attrs, "optsize") || llrm_mir::memory::has(&function.attrs, "minsize") {
                4
            } else {
                8
            };
        if u64::try_from(bytes).is_ok_and(|bytes| inline_widths(bytes, 4, limit, volatile).is_some()) {
            return false;
        }
    }
    if align(0) < 4 || align(1) < 4 {
        return true;
    }
    match instruction.operands[2] {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => bits > INLINE_LIMIT,
            ConstantKind::Zero | ConstantKind::Null => false,
            _ => true,
        },
        _ => true,
    }
}
