//! Static locals: the stack objects of a function that cannot be re-entered
//! are global storage.
//!
//! # How it works
//! A function marked `norecurse` (the pin infers it: no cycle of calls, no
//! call through a pointer or to a declaration that may call back) has one
//! activation at a time, so each of its fixed-size `alloca`s in the entry block
//! can be a zero-initialised internal global of its own. Every access is then
//! an absolute address (`lda g,y`) where the soft stack reaches an element
//! through `rc0` plus a constant plus the index, a 16-bit add per access, and
//! the function loses its frame (`ADJSP` both ways). `lifetime` markers on the
//! object go with it. Contents at entry are unspecified for an `alloca`, so
//! the zero start is valid. It runs after the mid-end: nothing that reads
//! the initializer of an internal global can then take the zero for what
//! the function left from an earlier call.
//!
//! A function with an `alloca` outside the entry block, a counted `alloca`,
//! or a stack save keeps its stack. The objects of a module together stay
//! under [`BUDGET`] bytes of RAM.
//!
//! # Price
//! The start-up zeroing of the new BSS costs [`ZERO_CLOCKS`] a byte, once.
//! Where clocks count (any level but `-Os`, which counts code bytes only) a
//! function is converted when the clocks it saves exceed that: per call, the
//! frame ([`FRAME_CLOCKS`]) and [`ACCESS_CLOCKS`] for each access, an access in
//! a loop counting 10 per level of depth, and the calls counted the same way
//! from the call sites in the module (one where it has none: an exported
//! function nothing here calls is priced as called once). A function
//! called once, outside a loop, with a big array it touches twice, keeps the
//! stack.
//!
//! DEVIATE (6502-6): not in the pin; llvm-mos has it as `MOSStaticStackAlloc`.
//! Reason: the soft stack costs a 16-bit add per element access; fmult of the
//! X16 math library runs 21% faster and 404 B smaller with its three arrays
//! static. Exit: overlay of the frames of functions that are never live
//! together, then [`BUDGET`] is the deepest call chain's frames (2026-11-20).
//!
//! # Example
//! ```text
//! define i8 @f(i8 %n) norecurse {          @f.local0 = internal global [6 x i8] zeroinitializer
//!   %a = alloca [6 x i8]               =>  define i8 @f(i8 %n) norecurse {
//!   ... gep %a, %i ...                       ... gep @f.local0, %i ...
//! ```

use std::collections::BTreeMap;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops;
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::facts::Facts;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{GlobalKind, GlobalVariable, InstId, Linkage, Module, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::types::TypeId;

/// Bytes of RAM the static locals of a module may take together.
pub const BUDGET: u64 = 2048;

/// Clocks the start-up zeroing of a byte of BSS costs, once (cc65's `zerobss`
/// on sim65, measured: 15).
pub const ZERO_CLOCKS: f64 = 15.0;
/// Clocks a call spends on a soft-stack frame: `rs0` adjusted both ways.
pub const FRAME_CLOCKS: f64 = 30.0;
/// Clocks an access saves: the frame address is a 16-bit add and a pointer
/// pair (12 to 14), the static one is the operand.
pub const ACCESS_CLOCKS: f64 = 8.0;

/// What one function gives up: its allocas and the lifetime markers on them.
struct Plan {
    function: usize,
    objects: Vec<Object>,
}

struct Object {
    alloca: InstId,
    ty: TypeId,
    align: Option<u64>,
    bytes: u64,
    markers: Vec<InstId>,
}

/// The allocas of every function of `module` that cannot be re-entered, as
/// globals. How many objects became globals.
pub fn applied(
    module: &mut Module,
    speed: bool,
) -> Result<usize, String> {
    let layout = module.datalayout.as_deref().map_or_else(|| Ok(DataLayout::default()), DataLayout::parse)?;
    let mut plans = Vec::new();
    let mut left = BUDGET;
    let calls = if speed { call_weights(module) } else { BTreeMap::new() };
    for (index, global) in module.globals.iter().enumerate() {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.entry().is_none() || !Facts::of(&function.attrs).no_recurse() {
            continue;
        }
        let Some(objects) = objects_of(module, function, &layout) else { continue };
        let bytes: u64 = objects.iter().map(|one| one.bytes).sum();
        if objects.is_empty() || bytes > left {
            continue;
        }
        if speed {
            let calls = calls.get(&index).copied().unwrap_or(1.0);
            let saved = calls * (FRAME_CLOCKS + ACCESS_CLOCKS * accesses(function, &objects));
            if saved <= ZERO_CLOCKS * bytes as f64 {
                continue;
            }
        }
        left -= bytes;
        plans.push(Plan { function: index, objects });
    }
    let mut made = 0;
    for plan in plans {
        let function_name = module.globals[plan.function].name.as_deref().unwrap_or("f").to_owned();
        for (number, object) in plan.objects.iter().enumerate() {
            let zero = module.context.constant(Constant { ty: object.ty, kind: ConstantKind::Zero });
            let variable = GlobalVariable {
                ty: object.ty,
                constant: false,
                externally_initialized: false,
                initializer: Some(zero),
                align: object.align,
            };
            let mut name = format!("{function_name}.local{number}");
            let mut again = 0;
            while module.named(&name).is_some() {
                again += 1;
                name = format!("{function_name}.local{number}.{again}");
            }
            let global = module.add_variable(&name, variable, Linkage::Internal)?;
            let address = Operand::Constant(module.reference(global));
            let GlobalKind::Function(function) = &mut module.globals[plan.function].kind else { unreachable!() };
            let result = function.instruction(object.alloca).result.ok_or("an alloca without a result")?;
            function.erase_all(&object.markers)?;
            function.replace_all_uses_with(result, address);
            function.erase(object.alloca)?;
            made += 1;
        }
    }
    Ok(made)
}

/// The entry-block allocas of `function`, or none where it must keep its
/// stack.
fn objects_of(
    module: &Module,
    function: &llrm_mir::module::Function,
    layout: &DataLayout,
) -> Option<Vec<Object>> {
    let entry = function.entry()?;
    let mut objects = Vec::new();
    for block in function.layout() {
        for inst in function.iter_block(*block) {
            let instruction = function.instruction(inst);
            match instruction.opcode {
                Opcode::Alloca { allocated, align, address_space } => {
                    if *block != entry || address_space != 0 || !instruction.operands.is_empty() {
                        return None;
                    }
                    let result = instruction.result?;
                    let mut markers = Vec::new();
                    for one in function.users(result).iter() {
                        if matches!(
                            callee_intrinsic(module, function, one.user),
                            Some(Intrinsic::LifetimeStart | Intrinsic::LifetimeEnd)
                        ) {
                            markers.push(one.user);
                        }
                    }
                    objects.push(Object {
                        alloca: inst,
                        ty: allocated,
                        align,
                        bytes: layout.alloc_size(&module.context.types, allocated),
                        markers,
                    });
                }
                Opcode::Call(_) => {
                    if matches!(
                        callee_intrinsic(module, function, inst),
                        Some(Intrinsic::StackSave | Intrinsic::StackRestore)
                    ) {
                        return None;
                    }
                }
                _ => {}
            }
        }
    }
    Some(objects)
}

/// The intrinsic a call names.
fn callee_intrinsic(
    module: &Module,
    function: &llrm_mir::module::Function,
    inst: InstId,
) -> Option<Intrinsic> {
    let Operand::Constant(id) = *function.instruction(inst).operands.last()? else { return None };
    let ConstantKind::Global(global) = module.context.get(id).kind else { return None };
    module.globals.get(global.0 as usize)?.intrinsic()
}

/// How much an access in a block of loop depth `depth` counts: 10 a level.
fn weight(depth: usize) -> f64 {
    10f64.powi(depth.min(3) as i32)
}

/// The accesses of `objects` in one activation, an access counting for the
/// loops around it. The users of an object's address and of an address made
/// from it (a `getelementptr` or a cast) count.
fn accesses(
    function: &llrm_mir::module::Function,
    objects: &[Object],
) -> f64 {
    let depths = block_depths(function);
    let at = |inst: InstId| function.parent(inst).and_then(|block| depths.get(&cfg::id(block))).copied().unwrap_or(0);
    let mut total = 0.0;
    for object in objects {
        let Some(result) = function.instruction(object.alloca).result else { continue };
        let mut work = vec![result];
        let mut seen = 0;
        while let Some(value) = work.pop() {
            seen += 1;
            if seen > 64 {
                break;
            }
            for one in function.users(value).iter() {
                let user = function.instruction(one.user);
                match user.opcode {
                    Opcode::GetElementPtr { .. } | Opcode::Cast(_) => work.extend(user.result),
                    _ => total += weight(at(one.user)),
                }
            }
        }
    }
    total
}

fn block_depths(function: &llrm_mir::module::Function) -> BTreeMap<i64, usize> {
    loops::depth(&cfg::graph(function), function.entry().map(cfg::id))
}

/// For each defined function, how many times its call sites in the module
/// run per run of their callers, a call in a loop counting 10 a level.
fn call_weights(module: &Module) -> BTreeMap<usize, f64> {
    let mut found: BTreeMap<usize, f64> = BTreeMap::new();
    for global in &module.globals {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.entry().is_none() {
            continue;
        }
        let mut depths = None;
        for block in function.layout() {
            for inst in function.iter_block(*block) {
                let instruction = function.instruction(inst);
                if !matches!(instruction.opcode, Opcode::Call(_)) {
                    continue;
                }
                let Some(Operand::Constant(id)) = instruction.operands.last().copied() else { continue };
                let ConstantKind::Global(callee) = module.context.get(id).kind else { continue };
                let depths = depths.get_or_insert_with(|| block_depths(function));
                *found.entry(callee.0 as usize).or_default() +=
                    weight(depths.get(&cfg::id(*block)).copied().unwrap_or(0));
            }
        }
    }
    found
}
