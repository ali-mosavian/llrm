//! `simplifySwitchLookup`: a switch whose only job is to give constants to the
//! phis of one common block becomes arithmetic on its condition, behind a range
//! check where the default can run. A step of `simplifySwitch`
//! ([L8591](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L8591)).
//!
//! # LLVM structure
//! | LLVM | here |
//! |---|---|
//! | [`simplifySwitchLookup`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7567) | [`folded`], `Plan::of`, `Plan::apply` |
//! | [`getCaseResults`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6641), `constantFold`, `lookupConstant` | [`case_results`] |
//! | [`SwitchReplacement`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7007) | `Replacement` |
//! | [`shouldBuildLookupTable`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7429), `isTypeLegalForLookupTable`, `isSwitchDense`, `wouldFitInRegister` | `should_build_lookup_table` and friends |
//! | [`shouldUseSwitchConditionAsTableIndex`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7466) | `condition_is_index` |
//! | [`reuseTableCompare`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7502) | `table_compare_reused` |
//!
//! The legality questions (`isTypeLegal`, `fitsInLegalInteger`) are asked of
//! the target ([`Tti::is_type_legal`](llrm_mir::tti::Tti::is_type_legal)) and
//! the data layout, not answered here.
//!
//! # How it works
//! Each case is walked to the common block, folding what the case block
//! computes from the condition (`case_results`). A table of the results is
//! sized (`condition_is_index`, the offset of the lowest case), priced
//! (`should_build_lookup_table`) and, per phi, found to be one value, linear
//! in the index, a bit map in a register or cells of an array global. The phis
//! then take the arithmetic, behind a range check or, for holes with a default
//! that is not a constant, a bit mask.
//!
//! # Example
//! ```text
//!   switch i16 %x, label %d [ 0, a  1, b  2, c  3, e ]    %0 = icmp ult i16 %x, 4
//!   phi [10, a] [13, b] [16, c] [19, e] [0, d]        =>  %1 = mul nsw i16 %x, 3
//!                                                         %2 = add nsw i16 %1, 10
//!                                                         br i1 %0, label %lookup, label %d
//! ```
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | A table is a private constant from `Declared::table`, named `switch.table.N`, deduplicated, without the function's name or the preferred alignment of its cells; `shouldBuildLookupTablesForConstant` is not asked and a constant MIR cannot hold is not offered. |
//! | D2 | No branch weights on the range check (`setFittedBranchWeights`). |
//! | D3 | No `DomTreeUpdater`: callers recompute the tree. |
//! | D4 | A hole at index 0 of a table whose holes are poison takes the first result as the linear offset; the pin `cast<ConstantInt>`s the poison. |
//!
//! # Used by
//! [`crate::switchopt`], step 6 of `simplifySwitch`, under
//! `convert_switch_to_arithmetic` or `convert_switch_to_lookup_table`.

use llrm_mir::constantfolding::{self, Env};
use llrm_mir::constantrange::{ConstantRange, PreferredRangeType};
use llrm_mir::context::{Constant, ConstantExpr, ConstantId, ConstantKind, Context, mask, signed};
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::tti::Ty;
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::range::compute_constant_range;
use llrm_support::hash::HashMap;

use crate::edges;
use crate::simplifycfg::Options;
use crate::twoentryphi::Speculation;

/// Whether the `switch` ending `block` was rewritten.
pub fn folded(
    options: &Options,
    context: &Context,
    spec: &Speculation,
    declared: &mut Declared,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some(switch) = function.terminator(block) else { return false };
    let Some(plan) = Plan::of(options, context, spec, function, switch, block) else { return false };
    plan.apply(context, spec, declared, function, switch, block);
    true
}

/// `v` as an unsigned 64-bit number, saturating: `APInt::getLimitedValue`.
fn limited(bits: u128) -> u64 {
    u64::try_from(bits).unwrap_or(u64::MAX)
}

fn is_null(
    context: &Context,
    constant: ConstantId,
) -> bool {
    matches!(
        context.get(constant).kind,
        ConstantKind::Int(0) | ConstantKind::Null | ConstantKind::Zero
    )
}

fn is_all_ones(
    context: &Context,
    constant: ConstantId,
) -> bool {
    let Constant { ty, kind } = context.get(constant);
    matches!(
        (kind, context.types.int_bits(*ty)),
        (ConstantKind::Int(bits), Some(width)) if *bits == mask(width)
    )
}

fn is_poison(
    context: &Context,
    constant: ConstantId,
) -> bool {
    matches!(context.get(constant).kind, ConstantKind::Poison)
}

/// `isa<UndefValue>`, which a poison is.
fn is_undef_or_poison(
    context: &Context,
    constant: ConstantId,
) -> bool {
    matches!(
        context.get(constant).kind,
        ConstantKind::Undef | ConstantKind::Poison
    )
}

/// `validLookupTableConstant`
/// ([L6569](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L6569)):
/// the backend can initialize an array with it. An aggregate is not one;
/// pointer casts and in-bounds GEPs of a valid constant are. D1.
fn valid_lookup_table_constant(
    context: &Context,
    constant: ConstantId,
) -> bool {
    match context.get(constant).kind {
        ConstantKind::Int(_)
        | ConstantKind::Float(_)
        | ConstantKind::Null
        | ConstantKind::Undef
        | ConstantKind::Poison
        | ConstantKind::Global(_) => true,
        ConstantKind::Expr(ConstantExpr::GetElementPtr { inbounds: true, ref operands, .. }) => {
            valid_lookup_table_constant(context, operands[0])
        }
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::BitCast | CastOp::AddrSpaceCast, value }) => {
            valid_lookup_table_constant(context, value)
        }
        _ => false,
    }
}

fn int_bits(
    context: &Context,
    constant: ConstantId,
) -> Option<u128> {
    match context.get(constant).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

/// `SwitchInst::defaultDestUnreachable`
/// ([Instructions.h:3508](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Instructions.h#L3508)).
pub(crate) fn default_dest_unreachable(
    function: &Function,
    default: BlockId,
) -> bool {
    function
        .iter_block(default)
        .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
        .is_some_and(|inst| function.instruction(inst).opcode == Opcode::Unreachable)
}

/// The case values (as bits) and destinations of `switch`, in order.
pub(crate) fn switch_cases(
    context: &Context,
    function: &Function,
    switch: InstId,
) -> Vec<(u128, BlockId)> {
    function.instruction(switch).operands[2..]
        .chunks(2)
        .map(|pair| match (pair[0], pair[1]) {
            (Operand::Constant(value), Operand::Block(to)) => {
                (int_bits(context, value).expect("a case value is an integer"), to)
            }
            other => unreachable!("a switch case is an integer and a block, not {other:?}"),
        })
        .collect()
}

pub(crate) fn switch_default(
    function: &Function,
    switch: InstId,
) -> BlockId {
    let Operand::Block(default) = function.instruction(switch).operands[1] else {
        unreachable!("a switch names its default")
    };
    default
}

/// `getCaseResults`: the constant each phi of the common block gets when
/// `switch` goes to `dest` on `case` (none: the default). The block the walk
/// ends in must be the common one, which the first walk names.
pub(crate) fn case_results(
    context: &Context,
    spec: &Speculation,
    function: &Function,
    switch: InstId,
    case: Option<u128>,
    mut dest: BlockId,
    common: &mut Option<BlockId>,
) -> Option<Vec<(InstId, ConstantId)>> {
    let env = Env { globals: &spec.outer.globals, layout: spec.layout };
    let condition = function.instruction(switch).operands[0];
    let mut pred = function.parent(switch).expect("a placed switch");
    let mut pool: HashMap<Operand, ConstantId> = HashMap::default();
    if let Some(bits) = case {
        let ty = function.operand_type(context, condition).expect("a switch condition has a type");
        pool.insert(condition, context.constant(Constant { ty, kind: ConstantKind::Int(bits) }));
    }
    let lookup = |pool: &HashMap<Operand, ConstantId>, operand: Operand| match operand {
        Operand::Constant(constant) => Some(constant),
        other => pool.get(&other).copied(),
    };

    // If the block is empty except for instructions that fold, go on to its
    // successor.
    let walked = dest;
    for inst in function.iter_block(walked) {
        let instruction = function.instruction(inst);
        if instruction.opcode.is_terminator() {
            dest = match (&instruction.opcode, &instruction.operands[..]) {
                (Opcode::Br, &[Operand::Block(next)]) | (Opcode::Switch, &[_, Operand::Block(next)]) => next,
                _ => return None,
            };
            pred = walked;
            continue;
        }
        let folded = if instruction.opcode == Opcode::Select {
            lookup(&pool, instruction.operands[0]).and_then(|condition| {
                if is_all_ones(context, condition) {
                    lookup(&pool, instruction.operands[1])
                } else if is_null(context, condition) {
                    lookup(&pool, instruction.operands[2])
                } else {
                    None
                }
            })
        } else {
            let operands: Option<Vec<_>> = instruction.operands.iter().map(|&one| lookup(&pool, one)).collect();
            operands.and_then(|operands| {
                constantfolding::constant_fold_inst_operands_at(context, env, function, inst, &operands)
            })
        };
        let (Some(constant), Some(result)) = (folded, instruction.result) else { break };
        // A use outside this block, or a phi slot of another predecessor,
        // would no longer be dominated once the instruction is bypassed.
        for one in function.users(result) {
            let user = function.instruction(one.user);
            if function.parent(one.user) == Some(walked) {
                continue;
            }
            if user.opcode == Opcode::Phi && user.operands[one.index as usize + 1] == Operand::Block(walked) {
                continue;
            }
            return None;
        }
        pool.insert(Operand::Value(result), constant);
    }

    let common_dest = *common.get_or_insert(dest);
    if dest != common_dest {
        return None;
    }
    let mut results = Vec::new();
    for phi in edges::phis(function, common_dest) {
        let operands = &function.instruction(phi).operands;
        let Some(at) = operands.chunks(2).position(|pair| pair[1] == Operand::Block(pred)) else { continue };
        results.push((phi, lookup(&pool, operands[2 * at])?));
    }
    (!results.is_empty()).then_some(results)
}

/// `isSwitchDense(NumCases, CaseRange, OptSize)`
/// ([L7367](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7367)):
/// at least 10% of the range has a case, 40% for size.
fn is_switch_dense(
    cases: u64,
    range: u64,
    opt_size: bool,
) -> bool {
    let min_density = if opt_size { 40 } else { 10 };
    if range >= u64::MAX / 100 {
        return false;
    }
    cases.wrapping_mul(100) >= range.wrapping_mul(min_density)
}

/// `SwitchReplacement::wouldFitInRegister`
/// ([L7325](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7325)):
/// `table_size` elements of `element` fit in a native integer.
fn would_fit_in_register(
    context: &Context,
    spec: &Speculation,
    table_size: u64,
    element: TypeId,
) -> bool {
    let Some(width) = context.types.int_bits(element) else { return false };
    if table_size >= u64::from(u32::MAX / width) {
        return false;
    }
    spec.layout.fits_in_legal_integer((table_size * u64::from(width)) as u32)
}

/// `isTypeLegalForLookupTable`
/// ([L7340](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7340)).
fn is_type_legal_for_lookup_table(
    context: &Context,
    spec: &Speculation,
    ty: TypeId,
) -> bool {
    if Ty::of(&context.types, ty).is_some_and(|one| spec.tti.is_type_legal(one)) {
        return true;
    }
    // Power-of-two integers of at least a byte that fit in a register: targets
    // usually load them.
    context
        .types
        .int_bits(ty)
        .is_some_and(|width| width >= 8 && width.is_power_of_two() && spec.layout.fits_in_legal_integer(width))
}

/// `shouldBuildLookupTable`
/// ([L7429](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7429)).
fn should_build_lookup_table(
    context: &Context,
    spec: &Speculation,
    cases: usize,
    table_size: u64,
    result_types: &[TypeId],
    opt_size: bool,
) -> bool {
    if cases as u64 > table_size {
        return false; // The size overflowed.
    }
    let all_fit = result_types.iter().all(|&ty| would_fit_in_register(context, spec, table_size, ty));
    if all_fit {
        return true;
    }
    let any_illegal = result_types.iter().any(|&ty| !is_type_legal_for_lookup_table(context, spec, ty));
    if any_illegal {
        return false;
    }
    is_switch_dense(cases as u64, table_size, opt_size)
}

/// `shouldUseSwitchConditionAsTableIndex`
/// ([L7466](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7466)):
/// whether to index by the condition itself, the table growing to start at 0.
fn condition_is_index(
    context: &Context,
    spec: &Speculation,
    min: u128,
    max: u128,
    width: u32,
    has_default_results: bool,
    result_types: &[TypeId],
) -> bool {
    if min == 0 {
        return true;
    }
    if signed(min, width) < 0 || limited(max) == u64::MAX || !has_default_results {
        return false;
    }
    result_types.iter().all(|&ty| would_fit_in_register(context, spec, limited(max) + 1, ty))
}

/// `a - b` overflows `width` bits as signed numbers: `APInt::ssub_ov`.
fn signed_sub_overflows(
    a: u128,
    b: u128,
    width: u32,
) -> bool {
    signed_overflows(signed(a, width).checked_sub(signed(b, width)), width)
}

/// `APInt::smul_ov`.
fn signed_mul_overflows(
    a: u128,
    b: u128,
    width: u32,
) -> bool {
    signed_overflows(signed(a, width).checked_mul(signed(b, width)), width)
}

fn signed_overflows(
    exact: Option<i128>,
    width: u32,
) -> bool {
    let Some(value) = exact else { return true };
    width < 128 && !(-(1i128 << (width - 1))..(1i128 << (width - 1))).contains(&value)
}

/// What gives a phi its value, `SwitchReplacement`'s kinds.
enum Replacement {
    /// `SingleValueKind`: every index gives this value.
    Single(ConstantId),
    /// `LinearMapKind`: `index * multiplier + offset`, in `ty`.
    Linear { ty: TypeId, offset: u128, multiplier: u128, wrapped: bool },
    /// `BitMapKind`: the elements of `ty`, `width` bits each, packed in `map`.
    BitMap { ty: TypeId, width: u32, map: u128 },
    /// `LookupTableKind`: the cells, of type `cell`, of an array global.
    Table { ty: TypeId, cell: TypeId, cells: Vec<ConstantId> },
}

impl Replacement {
    /// `SwitchReplacement::SwitchReplacement`
    /// ([L7090](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7090)):
    /// the table of `values` over `table_size` indices from `offset`, holes
    /// holding `default`. None: no representation (`InvalidKind`).
    fn of(
        context: &Context,
        spec: &Speculation,
        table_size: u64,
        offset: u128,
        width: u32,
        values: &[(u128, ConstantId)],
        default: Option<ConstantId>,
    ) -> Option<Replacement> {
        let value_type = context.get(values[0].1).ty;
        let mut contents: Vec<Option<ConstantId>> = vec![None; table_size as usize];
        // If all values in the table are equal, this is that value.
        let mut single = Some(values[0].1);
        for &(case, result) in values {
            let index = limited(case.wrapping_sub(offset) & mask(width));
            contents[index as usize] = Some(result);
            if let Some(one) = single
                && !is_poison(context, result)
                && result != one
            {
                single = if is_poison(context, one) { Some(result) } else { None };
            }
        }
        // Fill in any holes in the table with the default result.
        if (values.len() as u64) < table_size {
            let default = default.expect("a default value fills the holes");
            for hole in contents.iter_mut().filter(|one| one.is_none()) {
                *hole = Some(default);
            }
            // If the default value is poison, all the holes are poison.
            if single != Some(default) && !is_poison(context, default) {
                single = None;
            }
        }
        let contents: Vec<ConstantId> = contents.into_iter().map(|one| one.expect("every index is filled")).collect();
        if let Some(one) = single {
            return Some(Replacement::Single(one));
        }
        if let Some(linear) = Self::linear(context, &contents, values[0].1, value_type) {
            return Some(linear);
        }
        // If the values are integer constants and the table fits in a
        // register, build a bitmap.
        if would_fit_in_register(context, spec, table_size, value_type)
            && contents.iter().all(|&one| int_bits(context, one).is_some() || is_undef_or_poison(context, one))
        {
            let width = context.types.int_bits(value_type).expect("a bit map holds integers");
            let mut map = 0u128;
            // Undef values are set to zero.
            for &one in contents.iter().rev() {
                map = (map << width) | int_bits(context, one).unwrap_or(0);
            }
            return Some(Replacement::BitMap { ty: value_type, width, map });
        }
        // The remaining representation needs a global initializer.
        if !contents.iter().all(|&one| valid_lookup_table_constant(context, one)) {
            return None;
        }
        let mut contents = contents;
        let mut cell = value_type;
        if let Some(width) = context.types.int_bits(value_type) {
            let mut range = ConstantRange::empty(width);
            for &one in contents.iter().filter(|&&one| !is_undef_or_poison(context, one)) {
                let bits = int_bits(context, one).expect("an integer");
                range = range.union_with(&ConstantRange::single(width, bits), PreferredRangeType::Smallest);
            }
            // TODO in the pin: handle sign extension as well.
            let needed =
                spec.tti.minimum_lookup_table_entry_bit_width().max(range.get_active_bits().next_power_of_two());
            if needed < width {
                cell = context.types.int(needed);
                for one in &mut contents {
                    let kind = match &context.get(*one).kind {
                        ConstantKind::Int(bits) => ConstantKind::Int(*bits & mask(needed)),
                        other => other.clone(),
                    };
                    *one = context.constant(Constant { ty: cell, kind });
                }
            }
        }
        Some(Replacement::Table { ty: value_type, cell, cells: contents })
    }

    /// The `isa<IntegerType>(ValueType)` arm: the same distance between two
    /// consecutive values.
    fn linear(
        context: &Context,
        contents: &[ConstantId],
        first: ConstantId,
        ty: TypeId,
    ) -> Option<Replacement> {
        let width = context.types.int_bits(ty)?;
        let table_size = contents.len() as u64;
        let mut previous = 0u128;
        let mut distance = 0u128;
        let mut first_value = 0u128;
        // When the map is monotonic and signed overflow cannot happen at the
        // last index, the add and the mul get `nsw`.
        let mut non_monotonic = false;
        for (at, &constant) in contents.iter().enumerate() {
            // A poison is (probably) a hole: it takes the first value (D4).
            let value = int_bits(context, if is_poison(context, constant) { first } else { constant })?;
            if at == 0 {
                first_value = value;
            } else {
                let step = value.wrapping_sub(previous) & mask(width);
                if at == 1 {
                    distance = step;
                } else if step != distance {
                    return None;
                }
                let positive = signed(step, width) > 0;
                non_monotonic |= if positive {
                    signed(value, width) <= signed(previous, width)
                } else {
                    signed(value, width) > signed(previous, width)
                };
            }
            previous = value;
        }
        // `isIntN(width, table_size - 1)`, and no signed overflow of the last
        // index times the multiplier.
        let last = (table_size - 1) as i64 as i128;
        let may_wrap = !(width >= 64 || (-(1i128 << (width - 1))..(1i128 << (width - 1))).contains(&last))
            || signed_mul_overflows(distance, (table_size - 1) as u128, width);
        Some(Replacement::Linear { ty, offset: first_value, multiplier: distance, wrapped: non_monotonic || may_wrap })
    }
}

/// `simplifySwitchLookup`'s decision: everything it knows before it builds.
struct Plan {
    common_dest: BlockId,
    /// Each phi with its results by case, and what replaces it.
    phis: Vec<(InstId, Vec<(u128, ConstantId)>, Replacement, Option<ConstantId>)>,
    min_case: u128,
    max_case: u128,
    width: u32,
    cond_is_index: bool,
    table_size: u64,
    default_is_reachable: bool,
    has_default_results: bool,
    table_has_holes: bool,
    need_mask: bool,
}

impl Plan {
    fn of(
        options: &Options,
        context: &Context,
        spec: &Speculation,
        function: &Function,
        switch: InstId,
        block: BlockId,
    ) -> Option<Plan> {
        let _ = block;
        // Ignore switches with less than three cases. Lookup tables will not
        // make them faster, so we don't analyze them.
        let cases = switch_cases(context, function, switch);
        if cases.len() < 3 {
            return None;
        }
        let condition = function.instruction(switch).operands[0];
        let cond_ty = function.operand_type(context, condition)?;
        let width = context.types.int_bits(cond_ty)?;
        let default_dest = switch_default(function, switch);
        let opt_size =
            function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(f) if f == "optsize" || f == "minsize"));

        // The result for each case value and phi of the common destination,
        // and the min and max case values.
        let (mut min_case, mut max_case) = (cases[0].0, cases[0].0);
        let mut common_dest = None;
        let mut lists: Vec<(InstId, Vec<(u128, ConstantId)>)> = Vec::new();
        let mut listed: IdMap<InstId, usize> = IdMap::new();
        for &(value, dest) in &cases {
            if signed(value, width) < signed(min_case, width) {
                min_case = value;
            }
            if signed(value, width) > signed(max_case, width) {
                max_case = value;
            }
            for (phi, result) in case_results(context, spec, function, switch, Some(value), dest, &mut common_dest)? {
                let at = *listed.get_or_insert_with(phi, || {
                    lists.push((phi, Vec::new()));
                    lists.len() - 1
                });
                lists[at].1.push((value, result));
            }
        }
        let common_dest = common_dest.expect("a case names the common destination");

        // If the table has holes, we need a constant result for the default
        // case or a bitmask that fits in a register.
        let default_results = case_results(context, spec, function, switch, None, default_dest, &mut Some(common_dest));
        let has_default_results = default_results.is_some();
        let defaults: IdMap<InstId, ConstantId> = default_results.unwrap_or_default().into_iter().collect();

        let phi_type = |phi: InstId| function.value(function.instruction(phi).result.expect("a phi's value")).ty;
        let result_types: Vec<TypeId> = lists.iter().map(|(phi, _)| phi_type(*phi)).collect();

        let cond_is_index =
            condition_is_index(context, spec, min_case, max_case, width, has_default_results, &result_types);
        let (mut table_size, offset) = if cond_is_index {
            (limited(max_case).wrapping_add(1), 0)
        } else {
            (limited(max_case.wrapping_sub(min_case) & mask(width)).wrapping_add(1), min_case)
        };

        // If the default destination is unreachable, or if the lookup table
        // covers all values of the conditional variable, branch directly to
        // the lookup table block. Otherwise, check that the condition is
        // within the case range.
        let num_results = lists[0].1.len() as u64;
        let mut default_is_reachable = !default_dest_unreachable(function, default_dest);
        let table_has_holes = num_results < table_size;

        // If the table has holes but the default destination doesn't produce
        // any constant results, the entries of the holes are poison.
        let all_holes_are_poison = table_has_holes && !has_default_results;

        // If the default destination doesn't produce a constant result but is
        // still reachable, and the table has holes, a mask decides whether the
        // index loads from the table or jumps to the default case.
        let need_mask = all_holes_are_poison && default_is_reachable;
        if need_mask {
            // As an extra penalty for the validity test we require more cases.
            if cases.len() < 4 || !spec.layout.fits_in_legal_integer(table_size as u32) {
                return None;
            }
        }

        if !should_build_lookup_table(context, spec, cases.len(), table_size, &result_types, opt_size) {
            return None;
        }

        if cond_is_index && has_default_results {
            // Grow the table to cover all possible index values to avoid the
            // range check; the default result fills the new holes.
            let query = Query::bare(context, function, spec.layout);
            let range = compute_constant_range(&query, condition, false, 0);
            let upper = limited(range.upper);
            if !range.is_upper_wrapped()
                && result_types.iter().all(|&ty| would_fit_in_register(context, spec, upper, ty))
            {
                // A case index above the bound may be unreachable: the table
                // does not shrink.
                table_size = upper.max(table_size);
                default_is_reachable = false;
            }
        }

        let mut phis = Vec::new();
        for (phi, results) in lists {
            let ty = context.get(results[0].1).ty;
            // Any value fills the holes of a table whose holes are poison.
            let default = if all_holes_are_poison {
                Some(context.constant(Constant { ty, kind: ConstantKind::Poison }))
            } else {
                defaults.get(&phi).copied()
            };
            let replacement = Replacement::of(context, spec, table_size, offset, width, &results, default)?;
            phis.push((phi, results, replacement, default));
        }

        let any_tables = phis.iter().any(|(_, _, one, _)| matches!(one, Replacement::Table { .. }));
        let any_bit_maps = phis.iter().any(|(_, _, one, _)| matches!(one, Replacement::BitMap { .. }));
        // A few conditions prevent the generation of lookup tables: the target
        // does not support them, or the function says "no-jump-tables". Other
        // replacements, like the bit map, are not stopped by them.
        let no_jump_tables = options.no_jump_tables
            || function.attrs.iter().any(|attr| {
                matches!(
                    attr,
                    Attribute::Str(name, Some(value)) if name == "no-jump-tables" && value == "true"
                )
            });
        if any_tables && (!spec.tti.should_build_lookup_tables() || no_jump_tables) {
            return None;
        }
        // In the early pipeline, no lookup tables, bit maps or mask checks:
        // they may inhibit further optimization.
        if !options.convert_switch_to_lookup_table && (any_tables || any_bit_maps || need_mask) {
            return None;
        }

        Some(Plan {
            common_dest,
            phis,
            min_case,
            max_case,
            width,
            cond_is_index,
            table_size,
            default_is_reachable,
            has_default_results,
            table_has_holes,
            need_mask,
        })
    }

    /// The rest of `simplifySwitchLookup`: the blocks and instructions the
    /// plan names, then the switch gone.
    fn apply(
        self,
        context: &Context,
        spec: &Speculation,
        declared: &mut Declared,
        function: &mut Function,
        switch: InstId,
        block: BlockId,
    ) {
        let cond = function.instruction(switch).operands[0];
        let cond_ty = function.operand_type(context, cond).expect("a condition has a type");
        let void = function.instruction(switch).ty;
        let i1 = context.types.int(1);
        let int = |ty: TypeId, bits: u128| {
            let width = context.types.int_bits(ty).expect("an integer type");
            Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(bits & mask(width)) }))
        };
        let default_dest = switch_default(function, switch);
        let before = Position::Before(switch);
        let offset = if self.cond_is_index { 0 } else { self.min_case };

        // The index is the condition less the offset.
        let table_index = if self.cond_is_index {
            cond
        } else {
            // If the default is unreachable, all case values are s>= the
            // minimum: the subtraction cannot wrap.
            let may_wrap = self.default_is_reachable || signed_sub_overflows(self.max_case, self.min_case, self.width);
            let flags = if may_wrap { Flags::default() } else { Flags::NSW };
            emit(
                function,
                before,
                Opcode::Binary(BinaryOp::Sub),
                cond_ty,
                vec![cond, int(cond_ty, offset)],
                flags,
                "switch.tableidx",
            )
        };

        // The largest table the condition's type can index.
        let max_table_size = if self.width > 63 { u64::MAX } else { 1u64 << self.width };
        debug_assert!(max_table_size >= self.table_size, "a switch has no more entries than its type has values");
        let first = function.create_block(Some(if self.need_mask { "switch.hole_check" } else { "switch.lookup" }));
        function.insert_block_before(first, self.common_dest).expect("a new block");

        let covered = max_table_size == self.table_size;
        let mut range_check = None;
        let direct = !self.default_is_reachable || covered;
        if direct {
            let jump =
                function.create_instruction(Opcode::Br, void, vec![Operand::Block(first)], Flags::default(), None);
            function.insert(jump, before).expect("a placed switch");
        } else {
            let compare = emit(
                function,
                before,
                Opcode::ICmp(IntPredicate::Ult),
                i1,
                vec![table_index, int(cond_ty, u128::from(self.table_size))],
                Flags::default(),
                "",
            );
            let branch = function.create_instruction(
                Opcode::Br,
                void,
                vec![compare, Operand::Block(first), Operand::Block(default_dest)],
                Flags::default(),
                None,
            );
            function.insert(branch, before).expect("a placed switch");
            range_check = Some((branch, compare));
        }

        // The block that does the lookups.
        let mut lookup = first;
        if self.need_mask {
            // Before the lookup, the hole check. A bit of the mask is set for
            // every case; a clear bit is a hole, which goes to the default.
            lookup = function.create_block(Some("switch.lookup"));
            function.insert_block_before(lookup, self.common_dest).expect("a new block");
            let bits = (self.table_size.saturating_sub(1).max(7) + 1).next_power_of_two() as u32;
            let map_ty = context.types.int(bits);
            let mut table_mask = 0u128;
            for &(case, _) in &self.phis[0].1 {
                table_mask |= 1u128 << limited(case.wrapping_sub(offset) & mask(self.width));
            }
            let at = Position::End(first);
            let index = int_cast(context, function, at, table_index, map_ty, Flags::default(), "switch.maskindex");
            let shifted = emit(
                function,
                at,
                Opcode::Binary(BinaryOp::LShr),
                map_ty,
                vec![int(map_ty, table_mask), index],
                Flags::default(),
                "switch.shifted",
            );
            let low =
                emit(function, at, Opcode::Cast(CastOp::Trunc), i1, vec![shifted], Flags::default(), "switch.lobit");
            let branch = function.create_instruction(
                Opcode::Br,
                void,
                vec![low, Operand::Block(lookup), Operand::Block(default_dest)],
                Flags::default(),
                None,
            );
            function.insert(branch, at).expect("a placed block");
            edges::predecessor_added(function, default_dest, first, block);
        }

        if direct {
            // The phis are cached: none is deleted here.
            function.predecessor_removed(context, default_dest, block, true);
        }

        let at = Position::End(lookup);
        for (phi, results, replacement, default) in &self.phis {
            let result = match replacement {
                Replacement::Single(constant) => Operand::Constant(*constant),
                Replacement::Linear { ty, offset, multiplier, wrapped } => {
                    let (ty, offset, multiplier, wrapped) = (*ty, *offset, *multiplier, *wrapped);
                    let flags = if wrapped { Flags::default() } else { Flags::NSW };
                    let mut value =
                        int_cast(context, function, at, table_index, ty, Flags::default(), "switch.idx.cast");
                    if multiplier != 1 {
                        let operands = vec![value, int(ty, multiplier)];
                        value =
                            emit(function, at, Opcode::Binary(BinaryOp::Mul), ty, operands, flags, "switch.idx.mult");
                    }
                    if offset != 0 {
                        let operands = vec![value, int(ty, offset)];
                        value = emit(function, at, Opcode::Binary(BinaryOp::Add), ty, operands, flags, "switch.offset");
                    }
                    value
                }
                Replacement::BitMap { ty, width, map } => {
                    // The bit map's type, and the index in it: the index is
                    // below the table size, so truncating is safe.
                    let map_ty = context.types.int((self.table_size * u64::from(*width)) as u32);
                    let shift = int_cast(context, function, at, table_index, map_ty, Flags::default(), "switch.cast");
                    // wouldFitInRegister guarantees the product fits.
                    let mut no_wrap = Flags::NUW;
                    no_wrap.insert(Flags::NSW);
                    let operands = vec![shift, int(map_ty, u128::from(*width))];
                    let shift =
                        emit(function, at, Opcode::Binary(BinaryOp::Mul), map_ty, operands, no_wrap, "switch.shiftamt");
                    let operands = vec![int(map_ty, *map), shift];
                    let down = emit(
                        function,
                        at,
                        Opcode::Binary(BinaryOp::LShr),
                        map_ty,
                        operands,
                        Flags::default(),
                        "switch.downshift",
                    );
                    emit(function, at, Opcode::Cast(CastOp::Trunc), *ty, vec![down], Flags::default(), "switch.masked")
                }
                Replacement::Table { ty, cell, cells } => {
                    let array = context.types.intern(Type::Array { element: *cell, count: cells.len() as u64 });
                    let initial =
                        context.constant(Constant { ty: array, kind: ConstantKind::Aggregate(cells.clone()) });
                    let table = declared.table("switch.table", array, initial);
                    let address = context.types.ptr(0);
                    let base = context.constant(Constant { ty: address, kind: ConstantKind::Global(table) });
                    let index_bits = spec.layout.pointer(0).index_bits;
                    let index_ty = context.types.int(index_bits);
                    let old = self.width;
                    // The index is below the table size: a zext of it is
                    // non-negative where the table fits the type's positive
                    // half.
                    let flags = if index_bits > old && (cells.len() as u128 - 1) < (1u128 << (old - 1)) {
                        Flags::NNEG
                    } else {
                        Flags::default()
                    };
                    let index = int_cast(context, function, at, table_index, index_ty, flags, "");
                    let operands = vec![Operand::Constant(base), int(index_ty, 0), index];
                    let place = emit(
                        function,
                        at,
                        Opcode::GetElementPtr { source: array },
                        address,
                        operands,
                        Flags::INBOUNDS,
                        "switch.gep",
                    );
                    let load = Opcode::Load { align: None, volatile: false };
                    let loaded = emit(function, at, load, *cell, vec![place], Flags::default(), "switch.load");
                    if cell == ty {
                        loaded
                    } else {
                        emit(
                            function,
                            at,
                            Opcode::Cast(CastOp::ZExt),
                            *ty,
                            vec![loaded],
                            Flags::default(),
                            "switch.ext",
                        )
                    }
                }
            };
            // Re-use the switch table compare if possible.
            if let (false, true, Some((branch, compare)), Some(default)) =
                (self.table_has_holes, self.has_default_results, range_check, *default)
            {
                let phi_block = function.parent(*phi).expect("a placed phi");
                let value = function.instruction(*phi).result.expect("a phi's value");
                let mut users: Vec<InstId> = function.users(value).iter().map(|one| one.user).collect();
                users.dedup();
                for user in users {
                    table_compare_reused(context, spec, function, user, phi_block, branch, compare, default, results);
                }
            }
            let mut inputs = function.instruction(*phi).operands.clone();
            inputs.extend([result, Operand::Block(lookup)]);
            function.set_operands(*phi, inputs);
        }
        let jump = function.create_instruction(
            Opcode::Br,
            void,
            vec![Operand::Block(self.common_dest)],
            Flags::default(),
            None,
        );
        function.insert(jump, at).expect("a placed block");

        // Remove the switch.
        let successors: Vec<BlockId> = function
            .instruction(switch)
            .operands
            .iter()
            .skip(1)
            .step_by(2)
            .filter_map(|one| if let Operand::Block(to) = one { Some(*to) } else { None })
            .collect();
        for successor in successors.into_iter().filter(|&one| one != default_dest) {
            function.predecessor_removed(context, successor, block, false);
        }
        function.erase(switch).expect("a terminator defines nothing");
    }
}

/// `Builder.Create*` at `at`: the new instruction's value.
fn emit(
    function: &mut Function,
    at: Position,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
    flags: Flags,
    name: &str,
) -> Operand {
    let inst = function.create_instruction(opcode, ty, operands, flags, (!name.is_empty()).then_some(name));
    function.insert(inst, at).expect("a placed position");
    Operand::Value(function.instruction(inst).result.expect("an operation's value"))
}

/// `CreateIntCast(value, ty, false)`: a `zext`, a `trunc`, or `value`.
fn int_cast(
    context: &Context,
    function: &mut Function,
    at: Position,
    value: Operand,
    ty: TypeId,
    zext_flags: Flags,
    name: &str,
) -> Operand {
    let from = function.operand_type(context, value).and_then(|one| context.types.int_bits(one)).expect("an integer");
    let to = context.types.int_bits(ty).expect("an integer type");
    let op = match from.cmp(&to) {
        std::cmp::Ordering::Equal => return value,
        std::cmp::Ordering::Less => CastOp::ZExt,
        std::cmp::Ordering::Greater => CastOp::Trunc,
    };
    emit(
        function,
        at,
        Opcode::Cast(op),
        ty,
        vec![value],
        if op == CastOp::ZExt { zext_flags } else { Flags::default() },
        name,
    )
}

/// `reuseTableCompare`
/// ([L7502](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L7502)):
/// a compare of the phi with a constant that tells the default from every
/// case is the range check's own result, or its inverse.
#[expect(clippy::too_many_arguments)]
fn table_compare_reused(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    user: InstId,
    phi_block: BlockId,
    branch: InstId,
    range_compare: Operand,
    default: ConstantId,
    values: &[(u128, ConstantId)],
) {
    let Opcode::ICmp(predicate) = function.instruction(user).opcode else { return };
    // The compare must be in the phi's block, so that jump threading can do
    // its work afterwards.
    if function.parent(user) != Some(phi_block) {
        return;
    }
    let Operand::Constant(other) = function.instruction(user).operands[1] else { return };
    let i1 = context.types.int(1);
    let truth = context.constant(Constant { ty: i1, kind: ConstantKind::Int(1) });
    let falsity = context.constant(Constant { ty: i1, kind: ConstantKind::Int(0) });
    let fold = |value| {
        constantfolding::constant_fold_compare_inst_operands(
            context,
            &spec.outer.globals,
            spec.layout,
            predicate,
            value,
            other,
        )
    };
    // The compare with the default value is constant true or false...
    let Some(default_compare) = fold(default).filter(|&one| one == truth || one == falsity) else { return };
    // ...and every case's compare is the other.
    for &(_, value) in values {
        if fold(value).is_none_or(|one| one == default_compare || (one != truth && one != falsity)) {
            return;
        }
    }
    // The branch must dominate the phi's block: a simple check, enough here.
    let branch_block = function.parent(branch).expect("a placed branch");
    for pred in function.predecessors(phi_block) {
        let above: Vec<BlockId> = function.predecessors(pred).iter().copied().collect();
        if pred != branch_block && above.as_slice() != [branch_block] {
            return;
        }
    }
    let replaced = function.instruction(user).result.expect("a compare's value");
    if default_compare == falsity {
        // The compare yields the same result.
        function.replace_value(replaced, range_compare);
    } else {
        // The compare yields the same result, inverted.
        let inverted = emit(
            function,
            Position::Before(branch),
            Opcode::Binary(BinaryOp::Xor),
            i1,
            vec![range_compare, Operand::Constant(truth)],
            Flags::default(),
            "inverted.cmp",
        );
        function.replace_value(replaced, inverted);
    }
}

#[cfg(test)]
#[path = "switchlookup_tests.rs"]
mod tests;
