//! LLVM's `InterleavedAccessPass` (InterleavedAccessPass.cpp at
//! `d1106deb71cc`), store half, with AArch64's `lowerInterleavedStore`: a store
//! of a one-use `shufflevector` that re-interleaves two vectors becomes
//! `llvm.aarch64.neon.stN` of the `N` sub-vectors and the address; a load
//! whose users are all de-interleaving shuffles becomes `llvm.aarch64.neon.ldN`
//! and one `extractvalue` per shuffle.
//!
//! # Example
//! `store <32 x i8> (shufflevector <16 x i8> %a, <16 x i8> %b, <0, 16, 1, 17,
//! ..>), ptr %p` becomes `call void @llvm.aarch64.neon.st2.v16i8.p0(<16 x i8>
//! %a', <16 x i8> %b', ptr %p)` with `%a'`, `%b'` the sequential sub-shuffles,
//! which fold to `%a` and `%b`.
//!
//! # Runs
//! After SelectOptimize, before CodeGenPrepare, where the target
//! `selects_interleaved_accesses`.
//!
//! # Limits
//! DEVIATE IAP-1 (owner llrm-w11-coalescer-join, exit 2026-11-24): a load
//! with extract or binary-operator users (`tryReplaceExtracts`,
//! `replaceBinOpShuffles`); a sub-vector over 128 bits (several stores);
//! pointer elements; every 64-bit factor-2 store (the pin keeps those with
//! element 0 first and no paired store nearby); every nontemporal factor-2
//! store (the pin keeps them at `-Os`).

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{CallInfo, CastOp, Flags, Opcode, Tail};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::tti::Tti;
use llrm_mir::types::Type;

/// AArch64's `getMaxSupportedInterleaveFactor`.
const MAX_FACTOR: u32 = 4;

pub struct InterleavedAccess;

impl FunctionPass for InterleavedAccess {
    fn name(&self) -> &'static str {
        "interleavedaccess"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("interleaved-access")
    }

    fn adds_memory_operations(&self) -> bool {
        true
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let Some(tti) = analyses.outer().target().tti().filter(|tti| tti.selects_interleaved_accesses()) else {
            return PreservedAnalyses::all();
        };
        if lower_interleaved_stores(unit.context, unit.function, unit.declared, &*tti) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `isInterleaveMask`: the start index of each of `factor` members, if `mask`
/// interleaves them from `inputs` lanes.
fn interleave_starts(
    mask: &[i32],
    factor: usize,
    inputs: usize,
) -> Option<Vec<usize>> {
    let len = mask.len();
    if len % factor != 0 {
        return None;
    }
    let lane_len = len / factor;
    if !lane_len.is_power_of_two() {
        return None;
    }
    let mut starts = Vec::with_capacity(factor);
    for member in 0..factor {
        let mut saved = 0i32;
        let mut saved_defined = 0i32;
        let mut at = 0;
        while at + 1 < lane_len {
            let lane = at * factor + member;
            let (value, next) = (mask[lane], mask[lane + factor]);
            if value >= 0 && next >= 0 && value + 1 != next {
                break;
            }
            if value >= 0 && next < 0 {
                saved = value;
                saved_defined = 1;
            }
            if saved_defined > 0 && value < 0 {
                saved_defined += 1;
                if next >= 0 && saved + saved_defined != next {
                    break;
                }
            }
            at += 1;
        }
        if at + 1 < lane_len {
            return None;
        }
        let first = mask[member];
        let last = mask[(lane_len - 1) * factor + member];
        let start = if first >= 0 {
            first
        } else if last >= 0 {
            last - at as i32
        } else if saved_defined > 0 {
            saved - (lane_len as i32 - 1 - saved_defined)
        } else {
            0
        };
        if start < 0 || start as usize + lane_len > inputs {
            return None;
        }
        starts.push(start as usize);
    }
    Some(starts)
}

/// `isReInterleaveMask`: the factor of `mask`, 2 to `MAX_FACTOR`.
fn re_interleave_factor(
    mask: &[i32],
    inputs: usize,
) -> Option<usize> {
    if mask.len() < 4 {
        return None;
    }
    (2..=MAX_FACTOR as usize).find(|&factor| interleave_starts(mask, factor, inputs).is_some())
}

/// The suffix of an NEON intrinsic's name for a `<count x element>` vector.
fn vector_suffix(
    context: &Context,
    element: llrm_mir::types::TypeId,
    count: u32,
) -> Option<String> {
    let name = match context.types.get(element) {
        Type::Int(bits) => format!("i{bits}"),
        Type::Float(_) => match context.types.display(element).as_str() {
            "half" => "f16".to_owned(),
            "bfloat" => "bf16".to_owned(),
            "float" => "f32".to_owned(),
            "double" => "f64".to_owned(),
            _ => return None,
        },
        _ => return None,
    };
    Some(format!("v{count}{name}"))
}

/// Every store of a re-interleaving shuffle in `function` made a `stN` call;
/// whether any was.
pub fn lower_interleaved_stores(
    context: &mut Context,
    function: &mut Function,
    declared: &mut Declared,
    tti: &dyn Tti,
) -> bool {
    let stores: Vec<InstId> = function
        .layout()
        .iter()
        .flat_map(|&block| function.iter_block(block))
        .filter(|&inst| matches!(
            function.instruction(inst).opcode,
            Opcode::Store { volatile: false, .. }
        ))
        .collect();
    let loads: Vec<InstId> = function
        .layout()
        .iter()
        .flat_map(|&block| function.iter_block(block))
        .filter(|&inst| matches!(
            function.instruction(inst).opcode,
            Opcode::Load { volatile: false, .. }
        ))
        .collect();
    let mut changed = false;
    for store in stores {
        changed |= lower_store(context, function, declared, tti, store).is_some();
    }
    for load in loads {
        changed |= lower_load(context, function, declared, tti, load).is_some();
    }
    changed
}

/// `lowerInterleavedStore`, for the store `store`.
fn lower_store(
    context: &mut Context,
    function: &mut Function,
    declared: &mut Declared,
    tti: &dyn Tti,
    store: InstId,
) -> Option<()> {
    let [Operand::Value(stored), pointer] = function.instruction(store).operands[..] else { return None };
    if !function.instruction(store).metadata.is_empty() && has_nontemporal(function, store) {
        return None;
    }
    let ValueDef::Instruction(shuffle) = function.value(stored).def else { return None };
    let Opcode::ShuffleVector(mask) = &function.instruction(shuffle).opcode else { return None };
    let mask = mask.clone();
    if !function.users(stored).has_one_use() {
        return None;
    }
    let [op0, op1] = function.instruction(shuffle).operands[..] else { return None };
    let Type::Vector { element, count: operand_lanes } = *context.types.get(function.operand_type(context, op0)?)
    else {
        return None;
    };
    let factor = re_interleave_factor(&mask, operand_lanes as usize * 2)?;
    let lane_len = mask.len() / factor;
    let bits = context.types.int_bits(element).or_else(|| float_bits(context, element))?;
    if !tti.interleaved_access_legal(bits, lane_len as u32) || bits * lane_len as u32 > 128 {
        return None;
    }
    if mask.iter().all(|&lane| lane < 0) || (factor == 2 && bits * lane_len as u32 == 64) {
        return None;
    }
    let suffix = vector_suffix(context, element, lane_len as u32)?;
    let sub = context.types.intern(Type::Vector { element, count: lane_len as u32 });
    let space = match context.types.get(function.operand_type(context, pointer)?) {
        Type::Pointer(space) => *space,
        _ => return None,
    };
    let ptr = context.types.ptr(space);
    let mut parameters = vec![sub; factor];
    parameters.push(ptr);
    let void = context.types.void();
    let function_type = context.types.intern(Type::Function { returns: void, parameters, variadic: false });
    let callee = declared.declare(&format!("llvm.aarch64.neon.st{factor}.{suffix}.p{space}"), function_type);
    let callee = Operand::Constant(context.constant(Constant { ty: ptr, kind: ConstantKind::Global(callee) }));

    let mut operands = Vec::with_capacity(factor + 2);
    for member in 0..factor {
        let start = if mask[member] >= 0 {
            mask[member] as usize
        } else {
            // A member whose first lane is undef starts where its first
            // defined lane says; all undef: lane 0 (the lanes were undef
            // anyway).
            (1..lane_len)
                .map(|at| mask[at * factor + member])
                .enumerate()
                .find(|&(_, lane)| lane >= 0)
                .map_or(0, |(at, lane)| lane as usize - (at + 1))
        };
        let part = function.create_instruction(
            Opcode::ShuffleVector((start..start + lane_len).map(|lane| lane as i32).collect()),
            sub,
            vec![op0, op1],
            Flags::default(),
            None,
        );
        function.insert(part, Position::Before(store)).ok()?;
        operands.push(Operand::Value(function.instruction(part).result?));
    }
    operands.push(pointer);
    operands.push(callee);
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new(); factor + 1],
        attrs: Vec::new(),
        tail: Tail::default(),
    };
    let call = function.create_instruction(Opcode::Call(Box::new(info)), void, operands, Flags::default(), None);
    function.insert(call, Position::Before(store)).ok()?;
    function.erase(store).ok()?;
    function.erase(shuffle).ok()?;
    Some(())
}

/// `ShuffleVectorInst::isDeInterleaveMaskOfFactor`: the member `mask` takes.
fn de_interleave_index(
    mask: &[i32],
    factor: usize,
) -> Option<usize> {
    (0..factor)
        .find(|&index| mask.iter().enumerate().all(|(at, &lane)| lane < 0 || lane as usize == index + at * factor))
}

/// `isDeInterleaveMask`: the factor and member of `mask`, over a load of
/// `loaded` lanes.
fn de_interleave_factor(
    mask: &[i32],
    loaded: usize,
) -> Option<(usize, usize)> {
    if mask.len() < 2 {
        return None;
    }
    for factor in 2..=MAX_FACTOR as usize {
        if mask.len() * factor > loaded {
            return None;
        }
        if let Some(index) = de_interleave_index(mask, factor) {
            return Some((factor, index));
        }
    }
    None
}

/// `lowerInterleavedLoad`, for the load `load`: its users are all
/// de-interleaving shuffles.
fn lower_load(
    context: &mut Context,
    function: &mut Function,
    declared: &mut Declared,
    tti: &dyn Tti,
    load: InstId,
) -> Option<()> {
    let loaded_value = function.instruction(load).result?;
    let loaded_ty = function.instruction(load).ty;
    let Type::Vector { element, count: loaded_lanes } = *context.types.get(loaded_ty) else { return None };
    let pointer = *function.instruction(load).operands.first()?;
    let mut shuffles: Vec<InstId> = Vec::new();
    for user in function.users(loaded_value).iter() {
        let shuffle = user.user;
        let Opcode::ShuffleVector(_) = function.instruction(shuffle).opcode else { return None };
        let [Operand::Value(_), second] = function.instruction(shuffle).operands[..] else { return None };
        let Operand::Constant(second) = second else { return None };
        if user.index != 0 || !matches!(
            context.get(second).kind,
            ConstantKind::Undef | ConstantKind::Poison
        ) {
            return None;
        }
        if !shuffles.contains(&shuffle) {
            shuffles.push(shuffle);
        }
    }
    // `users` lists the newest first; the pin walks them the same way.
    let first = *shuffles.first()?;
    let Opcode::ShuffleVector(first_mask) = &function.instruction(first).opcode else { return None };
    let (factor, _) = de_interleave_factor(first_mask, loaded_lanes as usize)?;
    let vector = function.instruction(first).ty;
    let mut indices = Vec::with_capacity(shuffles.len());
    for &shuffle in &shuffles {
        let Opcode::ShuffleVector(mask) = &function.instruction(shuffle).opcode else { return None };
        if function.instruction(shuffle).ty != vector {
            return None;
        }
        indices.push(de_interleave_index(mask, factor)?);
    }
    let Type::Vector { count: lanes, .. } = *context.types.get(vector) else { return None };
    let bits = context.types.int_bits(element).or_else(|| float_bits(context, element))?;
    if !tti.interleaved_access_legal(bits, lanes) || bits * lanes > 128 {
        return None;
    }
    // Four members each widened by `uitofp` four-fold: left to shifts and
    // masks.
    if shuffles.len() == 4
        && shuffles.iter().all(|&shuffle| {
            let result = function.instruction(shuffle).result.expect("a shuffle");
            let users = function.users(result);
            users
                .only()
                .is_some_and(
                    |only| {
                        let user = function.instruction(only.user);
                        user.opcode == Opcode::Cast(CastOp::UIToFP) && scalar_bits(context, user.ty) == Some(bits * 4)
                    },
                )
        })
    {
        return None;
    }
    let suffix = vector_suffix(context, element, lanes)?;
    let space = match context.types.get(function.operand_type(context, pointer)?) {
        Type::Pointer(space) => *space,
        _ => return None,
    };
    let ptr = context.types.ptr(space);
    let result_ty = context.types.intern(Type::Struct { fields: vec![vector; factor], packed: false });
    let function_type =
        context.types.intern(Type::Function { returns: result_ty, parameters: vec![ptr], variadic: false });
    let callee = declared.declare(&format!("llvm.aarch64.neon.ld{factor}.{suffix}.p{space}"), function_type);
    let callee = Operand::Constant(context.constant(Constant { ty: ptr, kind: ConstantKind::Global(callee) }));
    let info = CallInfo {
        function_type,
        calling_convention: 0,
        return_attrs: Vec::new(),
        argument_attrs: vec![Vec::new()],
        attrs: Vec::new(),
        tail: Tail::default(),
    };
    let call = function.create_instruction(
        Opcode::Call(Box::new(info)),
        result_ty,
        vec![pointer, callee],
        Flags::default(),
        None,
    );
    function.insert(call, Position::Before(load)).ok()?;
    let aggregate = Operand::Value(function.instruction(call).result?);
    for (&shuffle, &index) in shuffles.iter().zip(&indices) {
        let part = function.create_instruction(
            Opcode::ExtractValue(vec![index as u32]),
            vector,
            vec![aggregate],
            Flags::default(),
            None,
        );
        function.insert(part, Position::Before(load)).ok()?;
        let old = function.instruction(shuffle).result?;
        function.replace_all_uses_with(old, Operand::Value(function.instruction(part).result?));
    }
    for shuffle in shuffles {
        function.erase(shuffle).ok()?;
    }
    function.erase(load).ok()?;
    Some(())
}

fn has_nontemporal(
    function: &Function,
    store: InstId,
) -> bool {
    function.instruction(store).metadata.iter().any(|(kind, _)| kind == "nontemporal")
}

/// `getScalarSizeInBits` of an integer or floating-point type or vector of one.
fn scalar_bits(
    context: &Context,
    ty: llrm_mir::types::TypeId,
) -> Option<u32> {
    let element = match context.types.get(ty) {
        Type::Vector { element, .. } => *element,
        _ => ty,
    };
    context.types.int_bits(element).or_else(|| float_bits(context, element))
}

fn float_bits(
    context: &Context,
    element: llrm_mir::types::TypeId,
) -> Option<u32> {
    match context.types.display(element).as_str() {
        "half" | "bfloat" => Some(16),
        "float" => Some(32),
        "double" => Some(64),
        _ => None,
    }
}

#[cfg(test)]
#[path = "interleavedaccess_pass_tests.rs"]
mod tests;
