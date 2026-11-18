//! A counted loop that walks by page ends on its index and a page count, not
//! on a second counter. The mechanism is `IndVarSimplify::linearFunctionTestReplace`
//! (`FindLoopCounter`, `genLoopLimit`) applied to the walk's own counter.
//!
//! # How it works
//! A loop that walks by page (see [`crate::pageindex`]) has a trip count
//! `k = 256 * page + index`. A header phi `c` that steps by one and is read
//! only by its exit test is `c = start + d * k` (`d` is 1 or -1), so the test
//! `c == end` holds on the trip `k = d * (end - start) - (c's step already
//! taken)`, the limit. The test becomes `index == lo(limit)`, and only on that
//! trip `left == 0`, where `left` starts at `hi(limit)` and drops a page where
//! the index wraps: one byte compare a trip, in place of the counter's own
//! step and a word test. The limit is made once before the loop.
//!
//! DEVIATE (6502-6): the pin's LFTR keeps the counter it chooses at the width
//! of the trip count and tests it whole (`FindLoopCounter`: "AR may not be a
//! narrower type"). The walk's index is one byte; the trip count's other byte
//! is the pages left, a second phi tested on the low byte's hit. The pin's pass
//! does not fire on this loop (`needsLFTR` finds `k2 == 0` canonical). Exit:
//! none, the index is narrower than the count by design.
//!
//! # Example
//! `memcmp`'s loop, `%k2 = add %k, -1 ; br %k2 == 0, done, head`, becomes
//!
//! ```text
//! head: %left = phi i8 [hi, pre], [%left, step], [%left1, bump]
//!       ...  br %i == lo, high, step
//! high: br %left == 0, done, step
//! bump: %left1 = add i8 %left, -1
//! ```
//!
//! # Runs
//! By [`crate::pageindex`] as it rewrites the loop, when asked to count pages.

use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, MetadataNode, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::TypeId;

/// How the loop's exit branch reads the counter's test.
#[derive(Clone, Copy)]
enum Reach {
    /// `br test, exit, stay`: the test is the whole condition.
    Alone,
    /// `br (other | test), exit, stay`: another condition leaves first.
    After(Operand),
}

/// A counter of the loop and the test that ends it.
pub(crate) struct Counter {
    phi: InstId,
    next: InstId,
    start: Operand,
    /// Whether it counts down.
    down: bool,
    /// Whether the test reads the stepped value.
    stepped: bool,
    end: Operand,
    ty: TypeId,
    test: InstId,
    /// The `or` or `select` that joins another condition to the test.
    joined: Option<InstId>,
    reach: Reach,
    branch: InstId,
    /// Whether the loop is left where the counter equals the end.
    equal_exits: bool,
}

impl Counter {
    pub(crate) fn phi(&self) -> InstId {
        self.phi
    }
}

/// What the page walk made, for the exit test.
pub(crate) struct Walk {
    pub preheader: BlockId,
    pub header: BlockId,
    pub join: BlockId,
    pub stepper: BlockId,
    pub bump: BlockId,
    pub index: InstId,
    pub index_bits: u32,
}

fn int(
    context: &mut Context,
    ty: TypeId,
    value: u128,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }))
}

fn constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    match operand {
        Operand::Constant(id) => match context.get(id).kind {
            ConstantKind::Int(bits) => Some(bits),
            _ => None,
        },
        _ => None,
    }
}

/// The counter of a loop that walks by page, if one has a test the walk can
/// take over. `latch` is the loop's only back edge.
pub(crate) fn find(
    context: &Context,
    function: &Function,
    header: BlockId,
    preheader: BlockId,
    latch: BlockId,
    index_bits: u32,
    in_loop: &dyn Fn(BlockId) -> bool,
) -> Option<Counter> {
    let outside = |operand: Operand| match operand {
        Operand::Constant(_) => true,
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(at) => function.parent(at).is_some_and(|block| !in_loop(block)),
            _ => true,
        },
        _ => false,
    };
    for phi in crate::edges::phis(function, header) {
        let ty = function.instruction(phi).ty;
        let Some(width) = context.types.int_bits(ty) else { continue };
        if width <= index_bits || width > 2 * index_bits {
            continue;
        }
        let Some(value) = function.instruction(phi).result else { continue };
        let [(first, from_first), (second, from_second)] = crate::pageindex::arms(function, phi)[..] else { continue };
        let (start, back) = if from_first == preheader && from_second == latch {
            (first, second)
        } else if from_second == preheader && from_first == latch {
            (second, first)
        } else {
            continue;
        };
        let Operand::Value(back) = back else { continue };
        let ValueDef::Instruction(next) = function.value(back).def else { continue };
        let ones = (1u128 << width) - 1;
        let down = match (&function.instruction(next).opcode, &function.instruction(next).operands[..]) {
            (Opcode::Binary(BinaryOp::Add), [stepped, by]) if *stepped == Operand::Value(value) => {
                match constant(context, *by) {
                    Some(1) => false,
                    Some(by) if by == ones => true,
                    _ => continue,
                }
            }
            (Opcode::Binary(BinaryOp::Sub), [stepped, by])
                if *stepped == Operand::Value(value) && constant(context, *by) == Some(1) =>
            {
                true
            }
            _ => continue,
        };
        // read by its own step and one equality test, nowhere else
        let mut tests = Vec::new();
        let mut kept = true;
        for (read, stepped) in [(value, false), (back, true)] {
            for one in function.users(read) {
                if one.user == next || one.user == phi {
                    continue;
                }
                match &function.instruction(one.user).opcode {
                    Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne) => tests.push((one.user, stepped)),
                    _ => kept = false,
                }
            }
        }
        let [(test, stepped)] = tests[..] else { continue };
        if !kept {
            continue;
        }
        let compare = function.instruction(test);
        let Opcode::ICmp(predicate) = compare.opcode.clone() else { continue };
        let counted = |operand: &Operand| *operand == Operand::Value(if stepped { back } else { value });
        let end = match &compare.operands[..] {
            [one, end] if counted(one) => *end,
            [end, one] if counted(one) => *end,
            _ => continue,
        };
        if !outside(end) {
            continue;
        }
        let Some(result) = compare.result else { continue };
        let [user] = function.users(result).into_iter().collect::<Vec<_>>()[..] else { continue };
        let Some(block) = function.parent(test) else { continue };
        let (joined, reach, branch) = match (&function.instruction(user.user).opcode, user.index) {
            (Opcode::Br, 0) => (None, Reach::Alone, user.user),
            (Opcode::Select, 2) | (Opcode::Binary(BinaryOp::Or), _)
                if predicate == IntPredicate::Eq && function.parent(user.user) == Some(block) =>
            {
                let operands = &function.instruction(user.user).operands;
                let other = match (&function.instruction(user.user).opcode, user.index) {
                    (Opcode::Select, _) if constant(context, operands[1]) == Some(1) => operands[0],
                    (Opcode::Select, _) => continue,
                    (_, at) => operands[1 - at as usize],
                };
                let Some(joined_result) = function.instruction(user.user).result else { continue };
                let [br] = function.users(joined_result).into_iter().collect::<Vec<_>>()[..] else { continue };
                if function.instruction(br.user).opcode != Opcode::Br || br.index != 0 {
                    continue;
                }
                (Some(user.user), Reach::After(other), br.user)
            }
            _ => continue,
        };
        if function.parent(branch) != Some(block) || !in_loop(block) {
            continue;
        }
        let [_, Operand::Block(taken), Operand::Block(other)] = function.instruction(branch).operands[..] else {
            continue;
        };
        let (exit, stay) = match (predicate, reach) {
            (IntPredicate::Eq, _) => (taken, other),
            _ => (other, taken),
        };
        if in_loop(exit) || exit == stay {
            continue;
        }
        let equal_exits = predicate == IntPredicate::Eq;
        return Some(Counter { phi, next, start, down, stepped, end, ty, test, joined, reach, branch, equal_exits });
    }
    None
}

/// `a - b`, folded when both are constants.
fn difference(
    context: &mut Context,
    function: &mut Function,
    at: Position,
    ty: TypeId,
    width: u32,
    a: Operand,
    b: Operand,
) -> Result<Operand, String> {
    let mask = (1u128 << width) - 1;
    match (constant(context, a), constant(context, b)) {
        (Some(a), Some(b)) => Ok(int(context, ty, a.wrapping_sub(b) & mask)),
        (_, Some(0)) => Ok(a),
        (_, Some(b)) => {
            let by = int(context, ty, b.wrapping_neg() & mask);
            put(function, at, Opcode::Binary(BinaryOp::Add), ty, vec![a, by])
        }
        _ => put(function, at, Opcode::Binary(BinaryOp::Sub), ty, vec![a, b]),
    }
}

/// A made instruction placed at `at`.
fn make(
    function: &mut Function,
    at: Position,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
) -> Result<InstId, String> {
    let made = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(made, at).map_err(|why| why.to_string())?;
    Ok(made)
}

/// Like [`make`], for an instruction with a result: that value.
fn put(
    function: &mut Function,
    at: Position,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
) -> Result<Operand, String> {
    let made = make(function, at, opcode, ty, operands)?;
    Ok(Operand::Value(function.instruction(made).result.ok_or("a result")?))
}

fn redirect(
    function: &mut Function,
    target: BlockId,
    from: BlockId,
    to: &[BlockId],
) {
    for phi in crate::edges::phis(function, target) {
        let arms: Vec<_> = crate::pageindex::arms(function, phi)
            .into_iter()
            .flat_map(|(value, one)| {
                if one == from { to.iter().map(|at| (value, *at)).collect() } else { vec![(value, one)] }
            })
            .collect();
        crate::pageindex::set_arms(function, phi, &arms);
    }
}

/// Ends `counter`'s loop on the walk's index and a page count, and drops the
/// counter.
pub(crate) fn apply(
    context: &mut Context,
    metadata: &mut Vec<MetadataNode>,
    function: &mut Function,
    counter: Counter,
    walk: &Walk,
) -> Result<(), String> {
    let width = context.types.int_bits(counter.ty).ok_or("an integer counter")?;
    let index_ty = context.types.int(walk.index_bits);
    let flag = context.types.int(1);
    let zero = int(context, index_ty, 0);
    let back = int(context, index_ty, (1 << walk.index_bits) - 1);
    let index = Operand::Value(function.instruction(walk.index).result.ok_or("an index")?);

    // the trip the test ends the loop on, before the loop: `d * (end - start)`
    // less the step a stepped test has already taken
    let before = Position::Before(function.terminator(walk.preheader).ok_or("a terminated preheader")?);
    let span = if counter.down {
        difference(context, function, before, counter.ty, width, counter.start, counter.end)?
    } else {
        difference(context, function, before, counter.ty, width, counter.end, counter.start)?
    };
    let taken = int(context, counter.ty, u128::from(counter.stepped));
    let limit = difference(context, function, before, counter.ty, width, span, taken)?;
    let byte = |context: &mut Context, function: &mut Function, shift: u128| -> Result<Operand, String> {
        if let Some(value) = constant(context, limit) {
            return Ok(int(context, index_ty, (value >> shift) & ((1 << walk.index_bits) - 1)));
        }
        let moved = if shift == 0 {
            limit
        } else {
            let by = int(context, counter.ty, shift);
            put(function, before, Opcode::Binary(BinaryOp::LShr), counter.ty, vec![limit, by])?
        };
        put(function, before, Opcode::Cast(CastOp::Trunc), index_ty, vec![moved])
    };
    let low = byte(context, function, 0)?;
    let high = byte(context, function, u128::from(walk.index_bits))?;

    // the pages left: `hi(limit)` less one a page, stepped in the bump and read
    // through the join as the bases are
    let first = function
        .iter_block(walk.header)
        .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
        .map_or(Position::End(walk.header), Position::Before);
    let page_phi = function.create_instruction(Opcode::Phi, index_ty, Vec::new(), Flags::default(), None);
    function.insert(page_phi, first).map_err(|why| why.to_string())?;
    let page = Operand::Value(function.instruction(page_phi).result.ok_or("a page")?);
    let bump_end = Position::Before(function.terminator(walk.bump).ok_or("a terminated bump")?);
    let page_on = put(function, bump_end, Opcode::Binary(BinaryOp::Add), index_ty, vec![page, back])?;
    if walk.join == walk.header {
        crate::pageindex::set_arms(
            function,
            page_phi,
            &[(high, walk.preheader), (page, walk.stepper), (page_on, walk.bump)],
        );
    } else {
        let top = function.iter_block(walk.join).next().ok_or("a terminated join")?;
        let joined = function.create_instruction(Opcode::Phi, index_ty, Vec::new(), Flags::default(), None);
        function.insert(joined, Position::Before(top)).map_err(|why| why.to_string())?;
        let ahead = Operand::Value(function.instruction(joined).result.ok_or("a joined page")?);
        crate::pageindex::set_arms(function, joined, &[(page, walk.stepper), (page_on, walk.bump)]);
        crate::pageindex::set_arms(function, page_phi, &[(high, walk.preheader), (ahead, walk.join)]);
    }

    // the test: the index against the limit's low byte, the page against its
    // high byte only on that trip
    // where the branch is now: pageindex may have split its block
    let block = function.parent(counter.branch).ok_or("a placed branch")?;
    let [_, Operand::Block(taken), Operand::Block(left)] = function.instruction(counter.branch).operands[..] else {
        return Err("a conditional exit".to_owned());
    };
    let (exit, stay) = if counter.equal_exits { (taken, left) } else { (left, taken) };
    let void = function.instruction(counter.branch).ty;
    let place = |function: &mut Function, name: &str, after: BlockId| {
        let block = function.create_block(Some(name));
        function.insert_block(block, Some(after)).map(|()| block).map_err(|why| why.to_string())
    };
    let high_block = place(function, "page-end", block)?;
    let low_block = match counter.reach {
        Reach::Alone => block,
        Reach::After(_) => place(function, "page-low", block)?,
    };
    let hit = put(
        function,
        Position::End(high_block),
        Opcode::ICmp(IntPredicate::Eq),
        flag,
        vec![page, zero],
    )?;
    make(
        function,
        Position::End(high_block),
        Opcode::Br,
        void,
        vec![hit, Operand::Block(exit), Operand::Block(stay)],
    )?;
    function.erase(counter.branch).map_err(|why| why.to_string())?;
    let low_hit = put(
        function,
        Position::End(low_block),
        Opcode::ICmp(IntPredicate::Eq),
        flag,
        vec![index, low],
    )?;
    let low_branch = make(
        function,
        Position::End(low_block),
        Opcode::Br,
        void,
        vec![low_hit, Operand::Block(high_block), Operand::Block(stay)],
    )?;
    // the low byte matches once in 256 trips
    crate::pageindex::weigh(context, metadata, function, low_branch, [1, 255])?;
    match counter.reach {
        Reach::Alone => {
            redirect(function, exit, block, &[high_block]);
            redirect(function, stay, block, &[block, high_block]);
        }
        Reach::After(other) => {
            make(
                function,
                Position::End(block),
                Opcode::Br,
                void,
                vec![other, Operand::Block(exit), Operand::Block(low_block)],
            )?;
            redirect(function, exit, block, &[block, high_block]);
            redirect(function, stay, block, &[low_block, high_block]);
        }
    }
    // the old test and the counter, which nothing reads now
    if let Some(joined) = counter.joined {
        function.erase(joined).map_err(|why| why.to_string())?;
    }
    function.erase(counter.test).map_err(|why| why.to_string())?;
    function.set_operands(counter.phi, Vec::new());
    function.erase(counter.next).map_err(|why| why.to_string())?;
    function.erase(counter.phi).map_err(|why| why.to_string())?;
    Ok(())
}
