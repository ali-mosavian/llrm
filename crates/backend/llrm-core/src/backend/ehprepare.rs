//! A QB personality's landing pad made one the runtime can enter, before
//! instruction selection, as LLVM's SjLjEHPrepare prepares setjmp/longjmp
//! unwinding.
//!
//! The runtime keeps one handler address per module (B$OEGA). On an error it
//! builds a frame of its own over the faulting one and jumps to that address;
//! B$RESN leaves the frame again, back on the faulting one, and continues at
//! the first statement-table row at or after the faulting call. So the
//! address registered is `LANDING`, which keeps ERR in `LANDED` and
//! calls B$RESN, and the landing pad is its function's
//! row. The pad is entered with nothing the function left in a register, so
//! each value live into it goes to the stack, as DemoteRegToStack puts it.

use std::collections::{BTreeMap, BTreeSet};

use llrm_hir::onerror::ERR;
pub use llrm_hir::onerror::{ONERROR, ONLOCALERROR, PERSONALITY};
use llrm_mir::context::{Constant, ConstantId, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, GlobalKind, GlobalVariable, InstId, Linkage, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{Attribute, BASIC, CallInfo, CastOp, Flags, Opcode};
use llrm_mir::{GlobalId, Module, TypeId};
/// The module's registered handler: keeps ERR, then resumes at the pad.
pub const LANDING: &str = "$QB$LANDING";
/// The ERR the landing kept, which the pad's selector reads.
pub const LANDED: &str = "$QB$LANDED";
/// The runtime's ON ERROR GOTO and RESUME NEXT.
const REGISTER: &str = "llrm.qb.B$OEGA";
const REGISTER_LOCAL: &str = "llrm.qb.B$OEGP";
const RESUME: &str = "llrm.qb.B$RESN";

/// Prepares each function of `module` with the QB personality, or refuses
/// what the handlers cannot yet serve.
pub fn prepared(module: &mut Module) -> Result<(), String> {
    // An intrinsic raises no BASIC error.
    let nounwind: BTreeSet<GlobalId> = module
        .functions()
        .filter(|(_, global, function)| {
            flagged(&function.attrs, "nounwind")
                || global.name.as_deref().is_some_and(llrm_mir::intrinsics::is_reserved)
        })
        .map(|(id, _, _)| id)
        .collect();
    let handled: Vec<GlobalId> =
        module.functions().filter(|(_, _, function)| personality(module, function)).map(|(id, _, _)| id).collect();
    let registrations: Vec<GlobalId> =
        [ONERROR, ONLOCALERROR].into_iter().filter_map(|one| module.named(one)).collect();
    let registers = |function: &Function| {
        function
            .walk()
            .any(|(_, inst)| callee(&module.context, function, inst).is_some_and(|one| registrations.contains(&one)))
    };
    // The module's handler takes an error raised anywhere in the module, on
    // the frame of the procedure that raised it; a procedure's own handler
    // only those its frame raises.
    let moduled = module
        .named(ONERROR)
        .is_some_and(
            |onerror| module.functions().any(|(_, _, function)| {
                function.walk().any(|(_, inst)| callee(&module.context, function, inst) == Some(onerror))
            }),
        );
    for (id, global, function) in module.functions() {
        if handled.contains(&id) || function.is_declaration() {
            continue;
        }
        let name = global.name.as_deref().unwrap_or_default();
        if registers(function) {
            return Err(format!("@{name} registers an error handler but has no {PERSONALITY}"));
        }
        if moduled && function.walk().any(|(_, inst)| unwinds(&module.context, &nounwind, function, inst)) {
            return Err(format!(
                "@{name} may raise an error, which the runtime hands to the module's handler on @{name}'s own frame: errors raised inside SUBs are not selected yet"
            ));
        }
    }
    let mut pads = Vec::new();
    for &id in &handled {
        let name = module.global(id).name.clone().unwrap_or_default();
        let why = |what: String| format!("@{name}: {what}");
        let function = module.global(id).function().expect("a function");
        if function.walk().any(|(_, inst)| function.instruction(inst).opcode == Opcode::Resume) {
            return Err(why(
                "a landing pad that resumes unwinding: an error its handler passes on is not selected yet".to_owned(),
            ));
        }
        // Where trapping may be on, every call that may raise is an invoke to
        // the pad: the runtime would land a call's error there with no site stored.
        let on = trapping(&module.context, function, &registrations);
        for &block in function.layout() {
            let mut trapped = on[&block];
            for &inst in function.block(block).instructions() {
                if trapped
                    && matches!(function.instruction(inst).opcode, Opcode::Call(_))
                    && unwinds(&module.context, &nounwind, function, inst)
                {
                    return Err(why(
                        "a call that may raise an error where ON ERROR is on, which is no invoke to the pad".to_owned(),
                    ));
                }
                trapped = trapping_after(&module.context, function, &registrations, inst, trapped);
            }
        }
        pads.push((name.clone(), pad(function).map_err(why)?));
    }
    let routines = if pads.iter().any(|(_, pad)| pad.is_some()) { Some(declare(module)?) } else { None };
    for (name, pad) in pads {
        let why = |what: String| format!("@{name}: {what}");
        let (context, function) = module.function_mut(&name).expect("a handled function");
        // Every call that may raise is an invoke to a pad: with none in the
        // module, nothing lands. The module handler's function registers
        // for every pad but has none.
        let Some(routines) = &routines else {
            unregistered(context, function, &registrations).map_err(why)?;
            continue;
        };
        registered(context, function, routines).map_err(why)?;
        let Some(pad) = pad else { continue };
        selected(context, function, pad, routines.landed).map_err(why)?;
        demote_phis(context, function, pad).map_err(why)?;
        demote_live(context, function, pad).map_err(why)?;
    }
    Ok(())
}

/// Whether trapping may be on where each block starts: off at entry, on
/// where the runtime lands, and as each ON ERROR leaves it.
/// The far address space of real mode, which BASIC's runtime is in.
fn far() -> u32 {
    llrm_x86_m16::layout().spaces.far
}

fn trapping(
    context: &Context,
    function: &Function,
    registrations: &[GlobalId],
) -> BTreeMap<BlockId, bool> {
    let mut on: BTreeMap<BlockId, bool> = function.layout().iter().map(|&block| (block, false)).collect();
    let mut pending: Vec<BlockId> = function.layout().to_vec();
    while let Some(block) = pending.pop() {
        let out = function
            .block(block)
            .instructions()
            .iter()
            .fold(on[&block], |trapped, &inst| trapping_after(context, function, registrations, inst, trapped));
        for next in function.successors(block) {
            if out && !on[&next] {
                on.insert(next, true);
                pending.push(next);
            }
        }
    }
    on
}

fn trapping_after(
    context: &Context,
    function: &Function,
    registrations: &[GlobalId],
    inst: InstId,
    trapped: bool,
) -> bool {
    let instruction = function.instruction(inst);
    if matches!(instruction.opcode, Opcode::LandingPad { .. }) {
        return true;
    }
    if !callee(context, function, inst).is_some_and(|one| registrations.contains(&one)) {
        return trapped;
    }
    !matches!(
        instruction.operands[0],
        Operand::Constant(one) if context.get(one).kind == ConstantKind::Int(0)
    )
}

fn flagged(
    attrs: &[Attribute],
    flag: &str,
) -> bool {
    attrs.iter().any(|one| matches!(one, Attribute::Flag(name) if name == flag))
}

fn personality(
    module: &Module,
    function: &Function,
) -> bool {
    function
        .personality
        .is_some_and(
            |one| match module.context.get(one).kind {
                ConstantKind::Global(global) => module.global(global).name.as_deref() == Some(PERSONALITY),
                _ => false,
            },
        )
}

/// The callee of a direct call or invoke.
fn callee(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<GlobalId> {
    let instruction = function.instruction(inst);
    if !matches!(instruction.opcode, Opcode::Call(_) | Opcode::Invoke(_)) {
        return None;
    }
    match instruction.operands.last() {
        Some(&Operand::Constant(one)) => match context.get(one).kind {
            ConstantKind::Global(global) => Some(global),
            _ => None,
        },
        _ => None,
    }
}

/// Whether `inst` may raise an error: an invoke, or a call that neither
/// the site nor the callee (one of `nounwind`) says is `nounwind`.
fn unwinds(
    context: &Context,
    nounwind: &BTreeSet<GlobalId>,
    function: &Function,
    inst: InstId,
) -> bool {
    match &function.instruction(inst).opcode {
        Opcode::Invoke(_) => true,
        Opcode::Call(info) => {
            !flagged(&info.attrs, "nounwind")
                && !callee(context, function, inst).is_some_and(|one| nounwind.contains(&one))
        }
        _ => false,
    }
}

/// The one block a landingpad starts, if any.
fn pad(function: &Function) -> Result<Option<BlockId>, String> {
    let pads: Vec<BlockId> =
        function.layout().iter().copied().filter(|&block| landing_pad(function, block).is_some()).collect();
    match pads[..] {
        [] => Ok(None),
        [one] => Ok(Some(one)),
        _ => Err("more than one landing pad, which the one module handler cannot choose between".to_owned()),
    }
}

fn landing_pad(
    function: &Function,
    block: BlockId,
) -> Option<InstId> {
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .find(|&one| matches!(function.instruction(one).opcode, Opcode::LandingPad { .. }))
}

/// What a registration becomes: the runtime's ON ERROR GOTO, B$OEGA with
/// the landing's far address, and its ON LOCAL ERROR GOTO, B$OEGP with its
/// offset, each by its function type; the landing, null, and the ERR it keeps.
struct Routines {
    onerror: Option<(GlobalId, TypeId, ConstantId)>,
    onlocal: Option<(GlobalId, TypeId, ConstantId)>,
    landing: ConstantId,
    null: ConstantId,
    landed: ConstantId,
}

/// The runtime routines the landing and the registrations call, and the
/// landing itself and the ERR it keeps.
fn declare(module: &mut Module) -> Result<Routines, String> {
    let types = &mut module.context.types;
    let (void, i16, far) = (types.void(), types.int(16), types.ptr(far()));
    let register = types.intern(llrm_mir::Type::Function { returns: void, parameters: vec![far], variadic: false });
    let local = types.intern(llrm_mir::Type::Function { returns: void, parameters: vec![i16], variadic: false });
    let asked = types.intern(llrm_mir::Type::Function { returns: i16, parameters: Vec::new(), variadic: false });
    let nothing = types.intern(llrm_mir::Type::Function { returns: void, parameters: Vec::new(), variadic: false });
    let mut routines = vec![(ERR, asked, &["nounwind"][..]), (RESUME, nothing, &["noreturn", "nounwind"][..])];
    let (onerror, onlocal) = (module.named(ONERROR), module.named(ONLOCALERROR));
    routines.extend(onerror.map(|_| (REGISTER, register, &[][..])));
    routines.extend(onlocal.map(|_| (REGISTER_LOCAL, local, &[][..])));
    for (name, ty, attrs) in routines {
        if let Some(one) = module.named(name) {
            let declared = module.global(one).function().map(|function| function.ty);
            if declared != Some(ty) {
                return Err(format!("@{name} is declared with another type"));
            }
            continue;
        }
        let id = module.add_function(name, ty, Linkage::External)?;
        runtime(module, id, attrs);
    }
    let zero = module.context.int(i16, 0);
    module.add_variable(
        LANDED,
        GlobalVariable { ty: i16, constant: false, initializer: Some(zero), align: None },
        Linkage::Internal,
    )?;
    let id = module.add_function(LANDING, nothing, Linkage::Internal)?;
    runtime(module, id, &["naked", "noreturn", "nounwind"]);
    let [err, resume, landed] = [ERR, RESUME, LANDED].map(|one| module.named(one).expect("declared"));
    let [err, resume, landed] = [err, resume, landed].map(|one| module.reference(one));
    let mut b = module.builder(id);
    let entry = b.block("");
    b.position(entry);
    let code = b.call_as(BASIC, asked, Operand::Constant(err), &[], "").expect("ERR");
    b.store(code, Operand::Constant(landed), false);
    b.call_as(BASIC, nothing, Operand::Constant(resume), &[], "");
    b.unreachable();
    let landing = module.reference(id);
    let null = module.context.constant(Constant { ty: far, kind: ConstantKind::Null });
    let mut routine = |registration: Option<GlobalId>, name: &str, ty: TypeId| {
        registration.map(|one| (one, ty, module.reference(module.named(name).expect("declared"))))
    };
    let (onerror, onlocal) = (routine(onerror, REGISTER, register), routine(onlocal, REGISTER_LOCAL, local));
    Ok(Routines { onerror, onlocal, landing, null, landed })
}

/// A far function by BASIC's convention, as the runtime's are.
fn runtime(
    module: &mut Module,
    id: GlobalId,
    attrs: &[&str],
) {
    let global = &mut module.globals[id.0 as usize];
    global.address_space = far();
    let GlobalKind::Function(function) = &mut global.kind else { unreachable!("a function") };
    function.calling_convention = BASIC;
    function.attrs.extend(attrs.iter().map(|one| Attribute::Flag((*one).to_owned())));
}

/// Each ON ERROR gone, where no call lands.
fn unregistered(
    context: &Context,
    function: &mut Function,
    registrations: &[GlobalId],
) -> Result<(), String> {
    let sites: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| callee(context, function, inst).is_some_and(|one| registrations.contains(&one)))
        .collect();
    sites.into_iter().try_for_each(|inst| function.erase(inst))
}

/// Each ON ERROR a call of B$OEGA with the landing or null; each ON LOCAL
/// ERROR one of B$OEGP with the landing's offset or 0.
fn registered(
    context: &mut Context,
    function: &mut Function,
    routines: &Routines,
) -> Result<(), String> {
    let registrations = [routines.onerror, routines.onlocal];
    let sites: Vec<(InstId, (GlobalId, TypeId, ConstantId))> = function
        .walk()
        .filter_map(|(_, inst)| {
            let called = callee(context, function, inst)?;
            registrations.into_iter().flatten().find(|&(one, _, _)| one == called).map(|routine| (inst, routine))
        })
        .collect();
    let void = context.types.void();
    for (inst, (registration, ty, routine)) in sites {
        let instruction = function.instruction(inst);
        if !matches!(instruction.opcode, Opcode::Call(_)) {
            return Err("an ON ERROR that is an invoke".to_owned());
        }
        let on = match instruction.operands[0] {
            Operand::Constant(one) => context.get(one).kind != ConstantKind::Int(0),
            _ => return Err("an ON ERROR whose handler is not constant".to_owned()),
        };
        let target = if Some(registration) == routines.onlocal.map(|(one, _, _)| one) {
            // The runtime keeps a procedure's handler as an offset in its code segment.
            let i16 = context.types.int(16);
            if on {
                let offset = function.create_instruction(
                    Opcode::Cast(CastOp::PtrToInt),
                    i16,
                    vec![Operand::Constant(routines.landing)],
                    Flags::default(),
                    None,
                );
                function.insert(offset, Position::Before(inst))?;
                Operand::Value(function.instruction(offset).result.expect("a value"))
            } else {
                Operand::Constant(context.int(i16, 0))
            }
        } else {
            Operand::Constant(if on { routines.landing } else { routines.null })
        };
        let info = CallInfo {
            function_type: ty,
            calling_convention: BASIC,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new()],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        let made = function.create_instruction(
            Opcode::Call(Box::new(info)),
            void,
            vec![target, Operand::Constant(routine)],
            Flags::default(),
            None,
        );
        function.insert(made, Position::Before(inst))?;
        function.erase(inst)?;
    }
    Ok(())
}

/// The pad's selector, ERR, read from what the landing kept.
fn selected(
    context: &mut Context,
    function: &mut Function,
    pad: BlockId,
    landed: ConstantId,
) -> Result<(), String> {
    let lp = landing_pad(function, pad).expect("a pad");
    let Some(value) = function.instruction(lp).result else { return Ok(()) };
    let i16 = context.types.int(16);
    for one in function.users(value).to_vec() {
        let user = function.instruction(one.user);
        if user.opcode != Opcode::ExtractValue(vec![1]) {
            return Err("a landing pad read other than for its selector".to_owned());
        }
        let (ty, result) = (user.ty, user.result.expect("a value"));
        let load = function.create_instruction(
            Opcode::Load { align: None, volatile: false },
            i16,
            vec![Operand::Constant(landed)],
            Flags::default(),
            Some("err"),
        );
        function.insert(load, Position::Before(one.user))?;
        let loaded = Operand::Value(function.instruction(load).result.expect("a value"));
        let widened = if ty == i16 {
            loaded
        } else {
            let cast =
                function.create_instruction(Opcode::Cast(CastOp::SExt), ty, vec![loaded], Flags::default(), None);
            function.insert(cast, Position::Before(one.user))?;
            Operand::Value(function.instruction(cast).result.expect("a value"))
        };
        function.replace_all_uses_with(result, widened);
        function.erase(one.user)?;
    }
    Ok(())
}

/// A stack slot for a value of `ty`, first in the entry block.
fn slot(
    context: &mut Context,
    function: &mut Function,
    ty: TypeId,
) -> Result<Operand, String> {
    let entry = function.entry().expect("a body");
    let first = function.block(entry).instructions()[0];
    let pointer = context.types.ptr(0);
    let alloca = function.create_instruction(
        Opcode::Alloca { allocated: ty, align: None, address_space: 0 },
        pointer,
        Vec::new(),
        Flags::default(),
        Some("demoted"),
    );
    function.insert(alloca, Position::Before(first))?;
    Ok(Operand::Value(function.instruction(alloca).result.expect("an address")))
}

fn store(
    context: &mut Context,
    function: &mut Function,
    value: Operand,
    slot: Operand,
    at: Position,
) -> Result<InstId, String> {
    let void = context.types.void();
    let made = function.create_instruction(
        Opcode::Store { align: None, volatile: false },
        void,
        vec![value, slot],
        Flags::default(),
        None,
    );
    function.insert(made, at)?;
    Ok(made)
}

fn load(
    function: &mut Function,
    ty: TypeId,
    slot: Operand,
    at: Position,
) -> Result<Operand, String> {
    let made = function.create_instruction(
        Opcode::Load { align: None, volatile: false },
        ty,
        vec![slot],
        Flags::default(),
        Some("reloaded"),
    );
    function.insert(made, at)?;
    Ok(Operand::Value(function.instruction(made).result.expect("a value")))
}

/// The first instruction of `block` after its phis and landing pad.
fn body_start(
    function: &Function,
    block: BlockId,
) -> Position {
    let found = function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .find(|&one| !matches!(
            function.instruction(one).opcode,
            Opcode::Phi | Opcode::LandingPad { .. }
        ));
    found.map_or(Position::End(block), Position::Before)
}

/// Each phi of the pad a slot its predecessors store into before they call,
/// as DemotePHIToStack makes it.
fn demote_phis(
    context: &mut Context,
    function: &mut Function,
    pad: BlockId,
) -> Result<(), String> {
    let phis: Vec<InstId> = function
        .block(pad)
        .instructions()
        .iter()
        .copied()
        .filter(|&one| function.instruction(one).opcode == Opcode::Phi)
        .collect();
    for phi in phis {
        let instruction = function.instruction(phi).clone();
        let slot = slot(context, function, instruction.ty)?;
        for pair in instruction.operands.chunks(2) {
            let Operand::Block(from) = pair[1] else { return Err("a phi with no block".to_owned()) };
            let terminator = function.terminator(from).expect("a terminator");
            store(context, function, pair[0], slot, Position::Before(terminator))?;
        }
        let at = body_start(function, pad);
        let reloaded = load(function, instruction.ty, slot, at)?;
        function.replace_all_uses_with(instruction.result.expect("a value"), reloaded);
        function.erase(phi)?;
    }
    Ok(())
}

/// Each block's live-in values: phis read on their edges, as liveness does.
fn live_in(function: &Function) -> BTreeMap<BlockId, BTreeSet<ValueId>> {
    let local = |operand: &Operand| match operand {
        Operand::Value(value) => Some(*value),
        _ => None,
    };
    let mut gen_kill: BTreeMap<BlockId, (BTreeSet<ValueId>, BTreeSet<ValueId>)> = BTreeMap::new();
    let mut edges: BTreeMap<(BlockId, BlockId), BTreeSet<ValueId>> = BTreeMap::new();
    for &block in function.layout() {
        let (mut used, mut defined) = (BTreeSet::new(), BTreeSet::new());
        for &inst in function.block(block).instructions() {
            let instruction = function.instruction(inst);
            if instruction.opcode == Opcode::Phi {
                for pair in instruction.operands.chunks(2) {
                    if let (Some(value), Operand::Block(from)) = (local(&pair[0]), pair[1]) {
                        edges.entry((from, block)).or_default().insert(value);
                    }
                }
            } else {
                used.extend(instruction.operands.iter().filter_map(local).filter(|one| !defined.contains(one)));
            }
            defined.extend(instruction.result);
        }
        gen_kill.insert(block, (used, defined));
    }
    let mut live: BTreeMap<BlockId, BTreeSet<ValueId>> =
        function.layout().iter().map(|&block| (block, BTreeSet::new())).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for &block in function.layout().iter().rev() {
            let mut out: BTreeSet<ValueId> = BTreeSet::new();
            for next in function.successors(block) {
                out.extend(live[&next].iter().copied());
                out.extend(edges.get(&(block, next)).into_iter().flatten().copied());
            }
            let (used, defined) = &gen_kill[&block];
            let mut entering: BTreeSet<ValueId> = out.difference(defined).copied().collect();
            entering.extend(used.iter().copied());
            if entering != live[&block] {
                live.insert(block, entering);
                changed = true;
            }
        }
    }
    live
}

/// Each value live into the pad kept in a slot: stored where it is made,
/// loaded where it is read, as DemoteRegToStack does.
fn demote_live(
    context: &mut Context,
    function: &mut Function,
    pad: BlockId,
) -> Result<(), String> {
    let values = live_in(function).remove(&pad).unwrap_or_default();
    let entry = function.entry().expect("a body");
    // An entry block's alloca is the frame's own address, in no register.
    let fixed = |function: &Function, value: ValueId| match function.value(value).def {
        ValueDef::Instruction(def) => {
            matches!(function.instruction(def).opcode, Opcode::Alloca { .. }) && function.parent(def) == Some(entry)
        }
        ValueDef::Argument(_) => false,
    };
    let values: Vec<ValueId> = values.into_iter().filter(|&one| !fixed(function, one)).collect();
    for value in values {
        let ty = function.value(value).ty;
        let slot = slot(context, function, ty)?;
        let at = match function.value(value).def {
            ValueDef::Argument(_) => {
                let entry = function.entry().expect("a body");
                let first = function
                    .block(entry)
                    .instructions()
                    .iter()
                    .copied()
                    .find(|&one| !matches!(function.instruction(one).opcode, Opcode::Alloca { .. }));
                first.map_or(Position::End(entry), Position::Before)
            }
            ValueDef::Instruction(def) => {
                let block = function.parent(def).expect("a placed definition");
                match function.instruction(def).opcode {
                    Opcode::Phi => body_start(function, block),
                    Opcode::Invoke(_) => {
                        let Some(&Operand::Block(normal)) = function.instruction(def).operands.iter().rev().nth(2)
                        else {
                            unreachable!("an invoke")
                        };
                        if function.predecessors(normal).len() != 1 {
                            return Err(
                                "an invoke's value live into the pad, continuing at a block others reach".to_owned()
                            );
                        }
                        body_start(function, normal)
                    }
                    _ => {
                        let list = function.block(block).instructions();
                        let at = list.iter().position(|&one| one == def).expect("in its block");
                        Position::Before(list[at + 1])
                    }
                }
            }
        };
        let kept = store(context, function, Operand::Value(value), slot, at)?;
        for one in function.users(value).to_vec() {
            if one.user == kept {
                continue;
            }
            let user = function.instruction(one.user).clone();
            let at = if user.opcode == Opcode::Phi {
                let Operand::Block(from) = user.operands[one.index as usize + 1] else { unreachable!("a phi's block") };
                let terminator = function.terminator(from).expect("a terminator");
                // The invoke that makes it reaches this phi on its normal edge.
                if matches!(
                    function.value(value).def,
                    ValueDef::Instruction(def) if def == terminator
                ) {
                    continue;
                }
                Position::Before(terminator)
            } else {
                Position::Before(one.user)
            };
            let reloaded = load(function, ty, slot, at)?;
            function.set_operand(one.user, one.index as usize, reloaded);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "ehprepare_tests.rs"]
mod tests;
