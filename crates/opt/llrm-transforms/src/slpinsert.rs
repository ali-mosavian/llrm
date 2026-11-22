//! `SLPVectorizer`'s buildvector seeds: a chain of `insertelement`s of
//! scalars is one vector when the tree of the scalars pays.
//!
//! # How it works
//! `vectorizeInserts` (l.35325) walks the inserts `vectorizeChainsInBlock`
//! collected since the last instruction with no user, last first. For each,
//! `findBuildAggregate` (l.34348) follows the chain through the first operand
//! (single-use inserts at constant indices) and `vectorizeInsertElementInst`
//! (l.34781) gives the inserted scalars, by element, to `tryToVectorizeList`
//! (l.30044): the root is the inserts, priced as the shuffles and inserts the
//! vector replaces (`getEntryCost`, l.16556) and made as the vector shuffled
//! into the chain's first operand (`vectorizeTree`, l.23698).
//!
//! DEVIATE SLP10 (owner arm64, exit 2026-12-01): the insert of a subvector
//! (`InsertSubvector`, a vector wider than a register with inserts in more than
//! one register) is not priced, so such a chain stays; the pass that tries
//! reductions rooted at an insert (`vectorizeHorReduction`) is not run here.
//!
//! # Runs
//! In `slpvectorize`, after the phis and before the store chains.
//!
//! # Used by
//! `slpvectorize`, `slptree`.
//!
//! # Example
//! A chain of `insertelement`s of scalars whose tree of operations pays becomes
//! one vector operation.
//!
//! ```text
//! %x0 = add i32 %a0, %b0
//! %x1 = add i32 %a1, %b1
//! %i0 = insertelement <2 x i32> poison, i32 %x0, i64 0
//! %i1 = insertelement <2 x i32> %i0, i32 %x1, i64 1
//! => %i1 = add <2 x i32> %va, %vb
//! ```

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Declared, Outer};
use llrm_mir::tti::{CostKind, LaneOp, ShuffleKind, Tti, VectorInstrContext, VectorTy};
use llrm_mir::types::Type;

use crate::slpphi::{Roots, Session};

/// What a chain of inserts is: where each scalar goes, and the vector they go
/// into.
#[derive(Clone, Debug)]
pub(crate) struct Inserts {
    /// The element each insert (in the order of the bundle) sets.
    pub indices: Vec<u32>,
    /// The elements of the vector built.
    pub elts: u32,
    /// What the first insert of the bundle inserts into.
    pub base: Operand,
}

/// `getElementIndex` of an `insertelement`: its constant index.
pub(crate) fn element_index(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<u32> {
    let data = function.instruction(inst);
    if data.opcode != Opcode::InsertElement {
        return None;
    }
    match data.operands.get(2)? {
        Operand::Constant(id) => match context.get(*id).kind {
            ConstantKind::Int(value) => u32::try_from(value).ok(),
            _ => None,
        },
        _ => None,
    }
}

/// The element count of a fixed vector type.
pub(crate) fn elements_of(
    context: &Context,
    ty: llrm_mir::types::TypeId,
) -> Option<u32> {
    match context.types.get(ty) {
        Type::Vector { count, .. } => Some(*count),
        _ => None,
    }
}

fn inst_of(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        _ => None,
    }
}

/// `findBuildAggregate` for `insertelement`: the inserts of the chain ending
/// at `last`, by the element they set; none unless two or more, each at a
/// constant index of its own.
pub(crate) fn build_aggregate(
    context: &Context,
    function: &Function,
    last: InstId,
) -> Option<Vec<InstId>> {
    let elts = elements_of(context, function.instruction(last).ty)? as usize;
    let mut found: Vec<Option<InstId>> = vec![None; elts];
    let mut at = last;
    loop {
        let index = element_index(context, function, at)? as usize;
        if index >= elts || found[index].is_some() {
            return None;
        }
        found[index] = Some(at);
        let Some(next) = inst_of(function, function.instruction(at).operands[0]) else { break };
        if function.instruction(next).opcode != Opcode::InsertElement || !has_one_use(function, next) {
            break;
        }
        at = next;
    }
    let inserts: Vec<InstId> = found.into_iter().flatten().collect();
    (inserts.len() >= 2).then_some(inserts)
}

fn has_one_use(
    function: &Function,
    inst: InstId,
) -> bool {
    function.instruction(inst).result.is_some_and(|result| function.users(result).iter().count() == 1)
}

/// Whether `operands` are extracts (or undef) that `isFixedVectorShuffle`
/// reads as a shuffle of at most two vectors: such a buildvector is left to
/// the shuffle.
fn extracts_form_a_shuffle(
    context: &Context,
    function: &Function,
    operands: &[Operand],
) -> bool {
    let is_undef = |operand: Operand| match operand {
        Operand::Constant(id) => matches!(
            context.get(id).kind,
            ConstantKind::Undef | ConstantKind::Poison
        ),
        _ => false,
    };
    let extract = |operand: Operand| {
        let inst = inst_of(function, operand)?;
        (function.instruction(inst).opcode == Opcode::ExtractElement).then_some(inst)
    };
    if !operands.iter().all(|operand| is_undef(*operand) || extract(*operand).is_some()) {
        return false;
    }
    let extracts: Vec<InstId> = operands.iter().filter_map(|operand| extract(*operand)).collect();
    if extracts.is_empty() {
        return false;
    }
    let size = extracts
        .iter()
        .filter_map(|inst| {
            let vector = function.operand_type(context, function.instruction(*inst).operands[0])?;
            elements_of(context, vector)
        })
        .max()
        .unwrap_or(0);
    let mut sources: Vec<Operand> = Vec::new();
    for inst in extracts {
        let operands = &function.instruction(inst).operands;
        let vector = operands[0];
        if is_undef(vector) {
            continue;
        }
        match operands[1] {
            Operand::Constant(id) => match context.get(id).kind {
                ConstantKind::Undef | ConstantKind::Poison => continue,
                ConstantKind::Int(index) if index >= u128::from(size) => continue,
                ConstantKind::Int(_) => {}
                _ => return false,
            },
            _ => return false,
        }
        if !sources.contains(&vector) {
            if sources.len() == 2 {
                return false;
            }
            sources.push(vector);
        }
    }
    true
}

/// `buildUseMask(VF, Mask, UndefsAsMask)`: the elements no lane of `mask`
/// names.
pub(crate) fn use_mask(
    vf: usize,
    mask: &[i32],
) -> Vec<bool> {
    let mut used = vec![true; vf];
    for (index, lane) in mask.iter().enumerate() {
        if *lane == crate::slpgather::POISON && index < vf {
            used[index] = false;
        }
    }
    used
}

/// `isUndefVector<IsPoisonOnly>(V, UseMask)`: which of the elements `mask`
/// leaves alone are undef (poison) in `value`, the inserts that build it
/// followed.
pub(crate) fn is_undef_vector(
    context: &Context,
    function: &Function,
    value: Operand,
    mask: &[bool],
    poison_only: bool,
) -> Vec<bool> {
    let size = if mask.is_empty() { 1 } else { mask.len() };
    let all = vec![true; size];
    let none = vec![false; size];
    let is_t = |kind: &ConstantKind| match kind {
        ConstantKind::Poison => true,
        ConstantKind::Undef => !poison_only,
        _ => false,
    };
    if let Operand::Constant(id) = value {
        if is_t(&context.get(id).kind) {
            return all;
        }
        let Some(ty) = function.operand_type(context, value) else { return none };
        let Some(count) = elements_of(context, ty) else { return none };
        let mut result = all;
        for index in 0..count as usize {
            let element_is_t = match &context.get(id).kind {
                ConstantKind::Aggregate(items) => items.get(index).is_some_and(|item| is_t(&context.get(*item).kind)),
                _ => false,
            };
            if !element_is_t && (mask.is_empty() || (index < mask.len() && !mask[index])) && index < size {
                result[index] = false;
            }
        }
        return result;
    }
    let Some(ty) = function.operand_type(context, value) else { return none };
    if elements_of(context, ty).is_none() {
        return none;
    }
    if mask.is_empty() {
        return none;
    }
    let mut result = all;
    let mut base = value;
    while let Some(inst) = inst_of(function, base)
        && function.instruction(inst).opcode == Opcode::InsertElement
    {
        base = function.instruction(inst).operands[0];
        let scalar = function.instruction(inst).operands[1];
        if matches!(scalar, Operand::Constant(id) if is_t(&context.get(id).kind)) {
            continue;
        }
        let Some(index) = element_index(context, function, inst) else { return none };
        if (index as usize) < mask.len() && !mask[index as usize] {
            result[index as usize] = false;
        }
    }
    if base == value {
        return none;
    }
    let rest = is_undef_vector(context, function, base, &vec![false; mask.len()], poison_only);
    result.iter().zip(&rest).map(|(a, b)| *a && *b).collect()
}

/// `getNumberOfParts`: the registers a vector of `count` lanes of `bits` is
/// split into where the target legalizes it.
fn number_of_parts(
    tti: &dyn Tti,
    bits: u32,
    count: u32,
) -> u32 {
    let register = tti.vector_register_bits().max(1);
    let parts = (bits * count).div_ceil(register);
    if parts == 0 || parts >= count || count % parts != 0 { 1 } else { parts }
}

fn valid(cost: llrm_mir::tti::Cost) -> Option<i64> {
    cost.valid.then_some(cost.value)
}

fn lane_bits(
    context: &Context,
    ty: llrm_mir::types::TypeId,
) -> Option<u32> {
    use llrm_mir::types::FloatKind;
    match context.types.get(ty) {
        Type::Int(bits) => Some(u32::from(*bits)),
        Type::Float(FloatKind::Float) => Some(32),
        Type::Float(FloatKind::Double) => Some(64),
        _ => None,
    }
}

/// The geometry of the inserts `getEntryCost` and `vectorizeTree` work from.
struct Shape {
    offset_beg: usize,
    offset_end: usize,
    vec_sz: usize,
    offset: usize,
    insert_vec_sz: usize,
    whole_subvector: bool,
    /// Where each element of the insert vector reads from the bundle.
    mask: Vec<i32>,
    identity: bool,
    /// `InsertMask`: the bundle's lane at each element it sets.
    insert_mask: Vec<i32>,
}

fn shape_of(
    tti: &dyn Tti,
    bits: u32,
    info: &Inserts,
) -> Shape {
    let elts = info.elts as usize;
    let scalars = info.indices.len();
    let parts = number_of_parts(tti, bits, info.elts) as usize;
    let mut insert_mask = vec![crate::slpgather::POISON; elts];
    let mut offset_beg = info.indices[0] as usize;
    let mut offset_end = offset_beg;
    insert_mask[offset_beg] = 0;
    for (lane, index) in info.indices.iter().enumerate().skip(1) {
        let index = *index as usize;
        if offset_beg > index {
            offset_beg = index;
        } else if offset_end < index {
            offset_end = index;
        }
        insert_mask[index] = lane as i32;
    }
    let mut vec_scalars_sz = elts.next_power_of_two();
    if parts > 0 && parts < elts {
        vec_scalars_sz = elts.div_ceil(parts).next_power_of_two();
    }
    let vec_sz = (1 + offset_end / vec_scalars_sz - offset_beg / vec_scalars_sz) * vec_scalars_sz;
    let offset = vec_scalars_sz * (offset_beg / vec_scalars_sz);
    let mut insert_vec_sz = (offset_end - offset_beg + 1)
        .next_power_of_two()
        .min(((offset_end - offset_beg + vec_scalars_sz) / vec_scalars_sz) * vec_scalars_sz);
    let whole_subvector = offset_beg == offset && (offset_end + 1) % vec_scalars_sz == 0;
    if offset_beg + insert_vec_sz > vec_sz {
        // `alignDown(OffsetBeg, VecSz, Offset)`.
        offset_beg = (offset_beg - offset) / vec_sz * vec_sz + offset;
        insert_vec_sz = vec_sz;
    }
    let mut mask = vec![crate::slpgather::POISON; insert_vec_sz];
    let mut identity = true;
    for (lane, index) in info.indices.iter().enumerate().take(scalars) {
        let at = *index as usize - offset_beg;
        identity &= at == lane;
        mask[at] = lane as i32;
    }
    Shape { offset_beg, offset_end, vec_sz, offset, insert_vec_sz, whole_subvector, mask, identity, insert_mask }
}

/// `getEntryCost` of an `InsertElement` bundle: the shuffles that place the
/// vector, less the inserts it replaces.
pub(crate) fn cost(
    context: &Context,
    function: &Function,
    tti: &dyn Tti,
    inserts: &[InstId],
    info: &Inserts,
) -> Option<i64> {
    let scalar_ty = function.operand_type(context, function.instruction(inserts[0]).operands[1])?;
    let lane = crate::slptree::lane_ty(context, scalar_ty)?;
    let bits = lane_bits(context, scalar_ty)?;
    let shape = shape_of(tti, bits, info);
    let elts = info.elts as usize;
    let src = VectorTy { lane, count: info.elts };
    let insert_ty = VectorTy { lane, count: shape.insert_vec_sz as u32 };
    let mut total = 0i64;
    if !shape.identity {
        total += valid(tti.shuffle_cost(
            ShuffleKind::PermuteSingleSrc,
            insert_ty,
            &shape.mask,
            crate::slptree::cost_kind(function),
        )?)?;
    }
    let in_mask = is_undef_vector(context, function, info.base, &use_mask(elts, &shape.insert_mask), false);
    // The inserts the vector replaces.
    for index in &info.indices {
        total -= valid(tti.vector_instr_cost(
            LaneOp::Insert,
            src,
            Some(*index),
            false,
            VectorInstrContext::None,
            crate::slptree::cost_kind(function),
        )?)?;
    }
    if !in_mask.iter().all(|one| *one) && info.indices.len() != elts && !shape.whole_subvector {
        if shape.insert_vec_sz != shape.vec_sz {
            // DEVIATE SLP10: `SK_InsertSubvector` is not priced.
            return None;
        }
        let mut mask = shape.mask.clone();
        for (at, slot) in mask.iter_mut().enumerate().take(shape.offset_beg - shape.offset) {
            *slot = if in_mask.get(at).copied().unwrap_or(true) { crate::slpgather::POISON } else { at as i32 };
        }
        for at in shape.offset_beg - shape.offset..=shape.offset_end - shape.offset {
            if mask[at] != crate::slpgather::POISON {
                mask[at] = (at + shape.vec_sz) as i32;
            }
        }
        for at in shape.offset_end + 1 - shape.offset..shape.vec_sz {
            mask[at] = if at >= in_mask.len() || in_mask[at] { crate::slpgather::POISON } else { at as i32 };
        }
        total += valid(tti.shuffle_cost(
            ShuffleKind::PermuteTwoSrc,
            insert_ty,
            &mask,
            crate::slptree::cost_kind(function),
        )?)?;
    }
    Some(total)
}

/// What `vectorizeTree` makes of the bundle's vector `vector`: the shuffles
/// that put its lanes at the elements the inserts set, in the chain's first
/// operand. `place` makes each shuffle, in order, from its operands and mask.
pub(crate) fn placed_vector(
    context: &mut Context,
    function: &mut Function,
    non_poisonous: bool,
    inserts: &[InstId],
    info: &Inserts,
    vector: Operand,
    place: &mut dyn FnMut(&mut Context, &mut Function, Vec<i32>, Operand, Operand) -> Operand,
) -> Operand {
    let elts = info.elts as usize;
    let scalars = inserts.len();
    let offset = info.indices[0] as usize;
    let mut mask = vec![crate::slpgather::POISON; elts];
    let mut identity = true;
    for (lane, index) in info.indices.iter().enumerate() {
        let at = *index as usize - offset;
        identity &= at == lane;
        mask[at] = lane as i32;
    }
    let vector_ty = function_type(context, function, vector);
    let poison = |context: &mut Context| {
        Operand::Constant(context.constant(llrm_mir::context::Constant { ty: vector_ty, kind: ConstantKind::Poison }))
    };
    let mut value = vector;
    if !identity || elts != scalars {
        let mut v2: Option<Operand> = None;
        let mut insert_mask = mask.clone();
        if elts != scalars && offset == 0 {
            // The inserts after the first, up the chain.
            let mut ins = inserts[0];
            loop {
                let Some(index) = element_index(context, function, ins) else { break };
                if insert_mask[index as usize] == crate::slpgather::POISON {
                    insert_mask[index as usize] = index as i32;
                }
                if !has_one_use(function, ins) {
                    break;
                }
                let result = function.instruction(ins).result.expect("a value");
                let Some(user) = function.users(result).iter().next().map(|used| (used.user, used.index)) else {
                    break;
                };
                if function.instruction(user.0).opcode != Opcode::InsertElement || user.1 != 0 {
                    break;
                }
                ins = user.0;
            }
            let first = first_insert(function, inserts);
            let base = function.instruction(first).operands[0];
            let used = use_mask(elts, &insert_mask);
            let first_poison = is_undef_vector(context, function, base, &used, true);
            let first_undef = is_undef_vector(context, function, base, &used, false);
            if !first_poison.iter().all(|one| *one) {
                let mut next = 0usize;
                for at in 0..elts {
                    if insert_mask[at] == crate::slpgather::POISON && !first_poison[at] && first_undef[at] {
                        if non_poisonous {
                            insert_mask[at] = if at < scalars { at as i32 } else { 0 };
                            continue;
                        }
                        if v2.is_none() {
                            v2 =
                                Some(Operand::Constant(context.constant(llrm_mir::context::Constant {
                                    ty: vector_ty,
                                    kind: ConstantKind::Undef,
                                })));
                        }
                        if next >= scalars {
                            next = scalars - 1;
                        }
                        insert_mask[at] = (scalars + next) as i32;
                        next += 1;
                    } else if insert_mask[at] != crate::slpgather::POISON && mask[at] == crate::slpgather::POISON {
                        insert_mask[at] = crate::slpgather::POISON;
                    }
                }
            }
        } else {
            insert_mask = mask.clone();
        }
        let second = v2.unwrap_or_else(|| poison(context));
        value = place(context, function, insert_mask, value, second);
    }
    let mut insert_mask = vec![crate::slpgather::POISON; elts];
    for (at, lane) in mask.iter().enumerate() {
        if *lane != crate::slpgather::POISON {
            insert_mask[offset + at] = at as i32;
        }
    }
    let used = use_mask(elts, &insert_mask);
    let first = first_insert(function, inserts);
    let base = function.instruction(first).operands[0];
    let first_undef = is_undef_vector(context, function, base, &used, false);
    if (!identity || offset != 0 || !first_undef.iter().all(|one| *one)) && elts != scalars {
        let first_poison = is_undef_vector(context, function, base, &used, true);
        if first_undef.iter().all(|one| *one) {
            if !slpgather_identity(&insert_mask, elts) {
                if !first_poison.iter().all(|one| *one) {
                    for at in 0..elts {
                        if insert_mask[at] == crate::slpgather::POISON && !first_poison[at] {
                            insert_mask[at] = (at + elts) as i32;
                        }
                    }
                }
                let second = if first_poison.iter().all(|one| *one) { poison(context) } else { base };
                value = place(context, function, insert_mask, value, second);
            }
        } else {
            for at in 0..elts {
                if insert_mask[at] == crate::slpgather::POISON {
                    insert_mask[at] = if first_poison[at] { crate::slpgather::POISON } else { at as i32 };
                } else {
                    insert_mask[at] += elts as i32;
                }
            }
            value = place(context, function, insert_mask, base, value);
        }
    }
    value
}

fn slpgather_identity(
    mask: &[i32],
    elts: usize,
) -> bool {
    mask.len() == elts && crate::slpgather::identity(mask)
}

fn function_type(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> llrm_mir::types::TypeId {
    function.operand_type(context, operand).expect("a typed vector")
}

/// The insert of the bundle whose first operand no insert of the bundle makes.
pub(crate) fn first_insert(
    function: &Function,
    inserts: &[InstId],
) -> InstId {
    *inserts
        .iter()
        .find(|insert| match inst_of(function, function.instruction(**insert).operands[0]) {
            Some(base) => !inserts.contains(&base),
            None => true,
        })
        .expect("a chain has a first insert")
}

/// The insert of the bundle no other insert of it reads.
pub(crate) fn last_insert(
    function: &Function,
    inserts: &[InstId],
) -> InstId {
    *inserts
        .iter()
        .find(|insert| {
            let result = function.instruction(**insert).result.expect("a value");
            !function.users(result).iter().any(|used| inserts.contains(&used.user))
        })
        .expect("a chain has a last insert")
}

/// Whether `inserts` are one run of the chain: each but the first takes the
/// one before it.
pub(crate) fn connected(
    function: &Function,
    inserts: &[InstId],
) -> bool {
    inserts
        .iter()
        .filter(|insert| {
            inst_of(function, function.instruction(**insert).operands[0]).is_none_or(|base| !inserts.contains(&base))
        })
        .count()
        == 1
}

/// The inserts of `function` that build a vector from scalars, made one
/// vector where the tree pays; whether any was.
pub(crate) fn run_function(
    context: &mut Context,
    layout: &DataLayout,
    declared: &mut Declared,
    function: &mut Function,
    outer: &Outer,
    tti: &dyn Tti,
) -> bool {
    let mut changed = false;
    for block in function.layout().to_vec() {
        // The inserts `vectorizeChainsInBlock` takes up to each instruction
        // that has no user.
        let mut groups: Vec<Vec<InstId>> = vec![Vec::new()];
        for inst in function.iter_block(block) {
            let data = function.instruction(inst);
            if data.opcode == Opcode::InsertElement {
                groups.last_mut().expect("a group").push(inst);
            }
            let unused = data.result.is_none_or(|result| function.users(result).iter().next().is_none());
            let void_or_call = data.result.is_none() || matches!(data.opcode, Opcode::Call(_) | Opcode::Invoke(_));
            if unused && void_or_call {
                groups.push(Vec::new());
            }
        }
        let mut session = Session::new(context, layout, declared, outer, tti, block, Roots::Inserts);
        for group in groups {
            changed |= vectorize_inserts(&mut session, function, &group);
        }
    }
    changed
}

/// `vectorizeInserts`: each insert, last first, as the end of a chain.
fn vectorize_inserts(
    session: &mut Session<'_>,
    function: &mut Function,
    group: &[InstId],
) -> bool {
    let mut changed = false;
    let mut tried: IdSet<InstId> = IdSet::new();
    for last in group.iter().rev() {
        if Session::deleted_inst(function, *last) || !tried.insert(*last) {
            continue;
        }
        for max_vf_only in [true, false] {
            if Session::deleted_inst(function, *last) {
                break;
            }
            changed |= vectorize_insert_element(session, function, *last, max_vf_only);
        }
    }
    changed
}

/// `vectorizeInsertElementInst`.
fn vectorize_insert_element(
    session: &mut Session<'_>,
    function: &mut Function,
    last: InstId,
    max_vf_only: bool,
) -> bool {
    let Some(inserts) = build_aggregate(session.context, function, last) else { return false };
    let operands: Vec<Operand> = inserts.iter().map(|insert| function.instruction(*insert).operands[1]).collect();
    if extracts_form_a_shuffle(session.context, function, &operands) {
        return false;
    }
    if max_vf_only && inserts.len() == 2 {
        return false;
    }
    session.try_list(function, &inserts, max_vf_only)
}
