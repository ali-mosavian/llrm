//! Whether two functions are the same code: the equivalence of LLVM's `FunctionComparator`, and the hash that
//! buckets functions before any pair is compared.
//!
//! # How it works
//! A translation of LLVM
//! [`FunctionComparator.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/FunctionComparator.cpp)
//! at d1106deb71cc. `compare` (L1041) walks both functions from the entry, a block pair at a time, taking the
//! successor pairs in order (L1041-L1081). Values are equal by serial number: an argument by its position, an
//! instruction or a block by the order the walk met it (`cmpValues`, L903), so two bodies that differ in names
//! and in layout but not in shape compare equal. A constant is equal to itself (the context interns them), and the
//! address of the function being compared is equal to the address of the other (`cmpValuesAllowingSelfRef`, L887),
//! so two directly recursive copies of one function match. `function_hash` is `FunctionComparator::functionHash`
//! (L1): the function type, the block and instruction counts and each instruction's opcode, so it never separates
//! two functions `equivalent` would join.
//!
//! # Deviation
//! DEVIATE MERGEFUNC-CMP (owner x86-m32, exit 2026-11-30): the pin answers a three-way order (`int`) so that a
//! `std::set` can sort functions; this answers a symmetric yes/no, and the caller buckets by `function_hash` and
//! compares a function against one representative of each class in its bucket. The pin's order is only a means to
//! the same classes. A function with an instruction the comparison does not know is equal to nothing: a
//! missed merge, never a wrong one.
//!
//! # Example
//! ```text
//!  define i32 @a(i32 %x) { %r = add i32 %x, 1  ret i32 %r }
//!  define i32 @b(i32 %y) { %s = add i32 %y, 1  ret i32 %s }   ; equivalent to @a
//!  define i32 @c(i32 %y) { %s = add i32 %y, 2  ret i32 %s }   ; not: another constant
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};

use llrm_analysis::globalsaa::embedded;
use llrm_mir::context::{ConstantKind, GlobalId};
use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Function, InstId, Module, Operand, ValueDef, ValueId};
use llrm_mir::module::{GlobalKind, Linkage, UnnamedAddr};
use llrm_mir::opcode::{Attribute, Opcode, Tail};
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};
use llrm_mir::types::Type;

/// `FunctionComparator::functionHash`: what two equivalent functions share.
pub fn function_hash(function: &Function) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    function.ty.hash(&mut state);
    function.calling_convention.hash(&mut state);
    function.layout().len().hash(&mut state);
    for &block in function.layout() {
        for inst in function.iter_block(block) {
            std::mem::discriminant(&function.instruction(inst).opcode).hash(&mut state);
        }
    }
    state.finish()
}

/// Whether `left` and `right`, bodies of the globals `left_id` and `right_id`, are the same code.
pub fn equivalent(
    module: &Module,
    left_id: GlobalId,
    left: &Function,
    right_id: GlobalId,
    right: &Function,
) -> bool {
    Comparator { module, left_id, right_id, left, right, values_left: IdMap::new(), values_right: IdMap::new(), blocks_left: IdMap::new(), blocks_right: IdMap::new(), ahead_left: IdMap::new(), ahead_right: IdMap::new() }
        .compare()
}

struct Comparator<'a> {
    module: &'a Module,
    left_id: GlobalId,
    right_id: GlobalId,
    left: &'a Function,
    right: &'a Function,
    /// A value's serial number on each side (`sn_mapL`, `sn_mapR`).
    values_left: IdMap<ValueId, u32>,
    values_right: IdMap<ValueId, u32>,
    blocks_left: IdMap<BlockId, u32>,
    blocks_right: IdMap<BlockId, u32>,
    /// Values read before the walk defines them (a loop's phi input), paired as they were first met.
    ahead_left: IdMap<ValueId, ValueId>,
    ahead_right: IdMap<ValueId, ValueId>,
}

impl Comparator<'_> {
    /// `compareSignature` (L990), then the walk (L1041).
    fn compare(mut self) -> bool {
        if !self.same_signature() {
            return false;
        }
        for (index, (&l, &r)) in self.left.parameters().iter().zip(self.right.parameters()).enumerate() {
            let sn = index as u32;
            self.values_left.insert(l, sn);
            self.values_right.insert(r, sn);
        }
        let (Some(&first_left), Some(&first_right)) = (self.left.layout().first(), self.right.layout().first()) else {
            return false;
        };
        let mut pending = vec![(first_left, first_right)];
        let mut met = HashMap::new();
        self.pair_blocks(first_left, first_right);
        met.insert(first_left, first_right);
        while let Some((l, r)) = pending.pop() {
            if !self.same_block(l, r) {
                return false;
            }
            let (succ_left, succ_right) = (self.left.successors(l), self.right.successors(r));
            if succ_left.len() != succ_right.len() {
                return false;
            }
            // in order: the first successor is compared first (a stack, pushed reversed)
            let mut next = Vec::new();
            for (&sl, &sr) in succ_left.iter().zip(succ_right.iter()) {
                match met.get(&sl) {
                    Some(&seen) => {
                        if seen != sr {
                            return false;
                        }
                    }
                    None => {
                        if !self.pair_blocks(sl, sr) {
                            return false;
                        }
                        met.insert(sl, sr);
                        next.push((sl, sr));
                    }
                }
            }
            pending.extend(next.into_iter().rev());
        }
        self.left.layout().len() == self.right.layout().len() && met.len() == self.left.layout().len()
    }

    /// Two blocks are the same when they were numbered together, or neither was numbered (`cmpValues` numbers a block on first sight).
    fn pair_blocks(&mut self, l: BlockId, r: BlockId) -> bool {
        match (self.blocks_left.get(&l), self.blocks_right.get(&r)) {
            (Some(m), Some(n)) => m == n,
            (None, None) => {
                let sn = self.blocks_left.len() as u32;
                self.blocks_left.insert(l, sn);
                self.blocks_right.insert(r, sn);
                true
            }
            _ => false,
        }
    }

    /// The function type, the convention and every attribute (`cmpAttrs`, L122).
    fn same_signature(&self) -> bool {
        let (l, r) = (self.left, self.right);
        l.ty == r.ty
            && l.calling_convention == r.calling_convention
            && l.attrs == r.attrs
            && l.return_attrs == r.return_attrs
            && l.parameter_attrs == r.parameter_attrs
            && l.personality == r.personality
            && l.parameters().len() == r.parameters().len()
    }

    /// `cmpBasicBlocks` (L949): the instructions in order, each by `cmpOperations` and its operands.
    fn same_block(&mut self, l: BlockId, r: BlockId) -> bool {
        let left: Vec<InstId> = self.left.iter_block(l).collect();
        let right: Vec<InstId> = self.right.iter_block(r).collect();
        if left.len() != right.len() {
            return false;
        }
        for (&a, &b) in left.iter().zip(&right) {
            if !self.same_instruction(a, b) {
                return false;
            }
            let (x, y) = (self.left.instruction(a).result, self.right.instruction(b).result);
            if let (Some(x), Some(y)) = (x, y) {
                // a value already read ahead must be defined here on both sides, or on neither
                if self.ahead_left.get(&x).is_some_and(|&paired| paired != y)
                    || self.ahead_right.get(&y).is_some_and(|&paired| paired != x)
                {
                    return false;
                }
                let sn = self.values_left.len() as u32;
                self.values_left.insert(x, sn);
                self.values_right.insert(y, sn);
            }
        }
        true
    }

    /// `cmpOperations` (L652): the operation, the types, the flags, the metadata and the operand count; then the operands.
    fn same_instruction(&mut self, a: InstId, b: InstId) -> bool {
        let (x, y) = (self.left.instruction(a), self.right.instruction(b));
        if x.opcode != y.opcode || x.ty != y.ty || x.flags != y.flags || x.operands.len() != y.operands.len() {
            return false;
        }
        let kept = |meta: &[(String, llrm_mir::module::MetadataId)]| {
            meta.iter().filter(|(kind, _)| kind != "dbg").cloned().collect::<Vec<_>>()
        };
        if kept(&x.metadata) != kept(&y.metadata) {
            return false;
        }
        // an instruction that holds a function's address as a callee or operand is handled by `same_operand`
        x.operands.iter().zip(&y.operands).all(|(&p, &q)| self.same_operand(p, q))
            && matches!(x.opcode, Opcode::Phi) == matches!(y.opcode, Opcode::Phi)
    }

    /// `cmpValues` (L903).
    fn same_operand(&mut self, p: Operand, q: Operand) -> bool {
        match (p, q) {
            (Operand::Constant(c), Operand::Constant(d)) => c == d || self.same_constant(c, d),
            (Operand::Block(c), Operand::Block(d)) => self.pair_blocks(c, d),
            (Operand::Value(a), Operand::Value(b)) => {
                let (m, n) = (self.values_left.get(&a), self.values_right.get(&b));
                match (m, n) {
                    (Some(m), Some(n)) => m == n,
                    // a value the walk defines later (a loop's phi input): paired now, checked where it is defined
                    (None, None) => {
                        matches!(self.left.value(a).def, ValueDef::Instruction(_))
                            && matches!(self.right.value(b).def, ValueDef::Instruction(_))
                            && self.left.value(a).ty == self.right.value(b).ty
                            && self.pair_ahead(a, b)
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Two values neither side has defined yet are the same value when they were paired before, or neither was.
    fn pair_ahead(&mut self, a: ValueId, b: ValueId) -> bool {
        match (self.ahead_left.get(&a), self.ahead_right.get(&b)) {
            (Some(&p), Some(&q)) => p == b && q == a,
            (None, None) => {
                self.ahead_left.insert(a, b);
                self.ahead_right.insert(b, a);
                true
            }
            _ => false,
        }
    }

    /// The address of one function against the address of the other (`cmpValuesAllowingSelfRef`, L887); otherwise equal ids.
    fn same_constant(&self, c: llrm_mir::context::ConstantId, d: llrm_mir::context::ConstantId) -> bool {
        let (x, y) = (self.module.context.get(c), self.module.context.get(d));
        x.ty == y.ty
            && matches!((&x.kind, &y.kind), (ConstantKind::Global(a), ConstantKind::Global(b)) if *a == self.left_id && *b == self.right_id)
    }
}

/// LLVM's `MergeFunctions` ([MergeFunctions.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/IPO/MergeFunctions.cpp)):
/// the functions of a module that are the same code become one.
///
/// Of a class of equivalent functions the first external one (else the first) is kept. A local one nothing takes the address of
/// (or whose address is `unnamed_addr`) is dropped and every reference goes to the kept one; any other becomes a thunk, a tail
/// call of the kept one with its own arguments (`writeThunk`). A thunk is a `jmp` once lowered.
///
/// DEVIATE MERGEFUNC-ALIAS (owner x86-m32, exit 2026-11-30): the pin writes an alias where the target allows one; this writes a
/// thunk. DEVIATE MERGEFUNC-ROUNDS: merging can make two callers equal, so the pass repeats while it merges (the pin's `Deferred`).
pub struct MergeFunctions;

impl ProgramPass for MergeFunctions {
    fn name(&self) -> &'static str {
        "mergefunc"
    }

    fn run(&mut self, program: &mut Program, analyses: &mut ProgramAnalyses) -> Result<(), String> {
        let mut changed = false;
        for module in &mut program.modules {
            while merge_functions(module, true) {
                changed = true;
            }
        }
        if changed {
            analyses.invalidate();
        }
        Ok(())
    }
}

/// `MergeFunctions` where a thunk is no trade (-O1 to -O3): the local functions nothing takes the address of are
/// dropped, the rest stay. A thunk saves bytes and costs a `jmp` a call, so only a drop is a win on both axes.
/// DEVIATE Z80-MERGEFUNC-LOCAL (owner z80, exit 2026-11-24): the pin runs no `MergeFunctions` at all.
pub struct MergeLocalFunctions;

impl ProgramPass for MergeLocalFunctions {
    fn name(&self) -> &'static str {
        "mergefunc-local"
    }

    fn run(&mut self, program: &mut Program, analyses: &mut ProgramAnalyses) -> Result<(), String> {
        let mut changed = false;
        for module in &mut program.modules {
            while merge_functions(module, false) {
                changed = true;
            }
        }
        if changed {
            analyses.invalidate();
        }
        Ok(())
    }
}

/// `isEligibleForMerging` (L1): a body that is the module's own and may be replaced.
fn eligible(module: &Module, id: GlobalId) -> bool {
    let global = module.global(id);
    let Some(function) = global.function() else { return false };
    let name = global.name.as_deref().unwrap_or("");
    !function.is_declaration()
        && matches!(global.linkage, Linkage::External | Linkage::Internal | Linkage::Private)
        && !name.starts_with("llvm.")
        && !function.attrs.iter().any(|a| matches!(a, Attribute::Flag(n) if n == "optnone" || n == "naked"))
}

fn local(linkage: Linkage) -> bool {
    matches!(linkage, Linkage::Internal | Linkage::Private)
}

/// The functions something other than a direct call names.
fn address_taken(module: &Module) -> BTreeSet<GlobalId> {
    let mut taken = BTreeSet::new();
    for alias in &module.aliases {
        taken.insert(alias.aliasee);
    }
    for global in &module.globals {
        match &global.kind {
            GlobalKind::Variable(variable) => {
                if let Some(initializer) = variable.initializer {
                    embedded(&module.context, initializer, &mut taken);
                }
            }
            GlobalKind::Function(function) => {
                for &block in function.layout() {
                    for inst in function.iter_block(block) {
                        let inst = function.instruction(inst);
                        let callee = matches!(inst.opcode, Opcode::Call(_)).then(|| inst.operands.len() - 1);
                        for (at, &operand) in inst.operands.iter().enumerate() {
                            let Operand::Constant(constant) = operand else { continue };
                            if Some(at) == callee && matches!(module.context.get(constant).kind, ConstantKind::Global(_)) {
                                continue;
                            }
                            embedded(&module.context, constant, &mut taken);
                        }
                    }
                }
            }
        }
    }
    taken
}

/// One round: whether any function was dropped or turned into a thunk.
fn merge_functions(module: &mut Module, thunks_allowed: bool) -> bool {
    let mut buckets: HashMap<u64, Vec<GlobalId>> = HashMap::new();
    for index in 0..module.globals.len() as u32 {
        let id = GlobalId(index);
        if eligible(module, id) {
            let function = module.global(id).function().expect("a function");
            buckets.entry(function_hash(function)).or_default().push(id);
        }
    }
    let mut classes: Vec<Vec<GlobalId>> = Vec::new();
    for ids in buckets.into_values().filter(|ids| ids.len() > 1) {
        let mut here: Vec<Vec<GlobalId>> = Vec::new();
        for id in ids {
            let function = module.global(id).function().expect("a function");
            let class = here.iter_mut().find(|class| {
                let head = class[0];
                equivalent(module, head, module.global(head).function().expect("a function"), id, function)
            });
            match class {
                Some(class) => class.push(id),
                None => here.push(vec![id]),
            }
        }
        classes.extend(here.into_iter().filter(|class| class.len() > 1));
    }
    classes.sort_by_key(|class| class[0]);
    let taken = address_taken(module);
    let mut gone: BTreeMap<GlobalId, GlobalId> = BTreeMap::new();
    let mut thunks: Vec<(GlobalId, GlobalId)> = Vec::new();
    for class in &classes {
        let keep = class.iter().copied().find(|&id| !local(module.global(id).linkage)).unwrap_or(class[0]);
        for &id in class.iter().filter(|&&id| id != keep) {
            let global = module.global(id);
            if local(global.linkage) && (!taken.contains(&id) || global.unnamed_addr == UnnamedAddr::Global) {
                gone.insert(id, keep);
            } else if thunks_allowed
                && !is_variadic(module, id)
                && !is_thunk_to(module, id, keep)
                && outweighs_thunk(module, id)
            {
                thunks.push((id, keep));
            }
        }
    }
    let changed = !gone.is_empty() || !thunks.is_empty();
    for (id, keep) in thunks {
        write_thunk(module, id, keep);
    }
    if !gone.is_empty() {
        module.context.renumber_globals(&|global| Some(gone.get(&global).copied().unwrap_or(global)));
        module.retain_globals(&|global| !gone.contains_key(&global));
    }
    changed
}

/// A body that is only its terminator (`ret`, `unreachable`) is no larger than the `jmp` that would replace it: value_of_valueless
/// grew 15 -> 17 B at m32 -Os. A body with any instruction beyond that is kept to the A/B: `call` plus `ret` of two arguments is
/// pushes, a call and a pop where the thunk is one `jmp` (sfm16 `__gtsf2`, 16 B against 5 B), so a count of two or three
/// instructions does not say the thunk is no smaller. The pin has no such check; this is part of DEVIATE MERGEFUNC-OS.
fn outweighs_thunk(module: &Module, id: GlobalId) -> bool {
    let function = module.global(id).function().expect("a function");
    function.layout().iter().map(|&block| function.iter_block(block).count()).sum::<usize>() > 1
}

fn is_variadic(module: &Module, id: GlobalId) -> bool {
    let function = module.global(id).function().expect("a function");
    matches!(module.context.types.get(function.ty), Type::Function { variadic: true, .. })
}

/// Whether `id` already is `write_thunk(id, keep)`'s result: a call of `keep` and a return.
fn is_thunk_to(module: &Module, id: GlobalId, keep: GlobalId) -> bool {
    let function = module.global(id).function().expect("a function");
    let [block] = function.layout() else { return false };
    let insts: Vec<_> = function.iter_block(*block).collect();
    let [call, ret] = insts[..] else { return false };
    let call = function.instruction(call);
    matches!(call.opcode, Opcode::Call(_))
        && matches!(function.instruction(ret).opcode, Opcode::Ret)
        && matches!(call.operands.last(), Some(&Operand::Constant(c)) if matches!(module.context.get(c).kind, ConstantKind::Global(g) if g == keep))
}

/// `writeThunk` (L1): the body of `id` becomes a tail call of `keep` with `id`'s arguments.
fn write_thunk(module: &mut Module, id: GlobalId, keep: GlobalId) {
    let callee = Operand::Constant(module.reference(keep));
    let function = module.global(id).function().expect("a function");
    let (ty, convention, attrs) = (function.ty, function.calling_convention, function.parameter_attrs.clone());
    let count = function.parameters().len();
    let GlobalKind::Function(body) = &mut module.globals[id.0 as usize].kind else { unreachable!("a function") };
    **body = body.declaration();
    let mut builder = module.builder(id);
    let entry = builder.block("entry");
    builder.position(entry);
    let arguments: Vec<Operand> = (0..count).map(|at| builder.parameter(at)).collect();
    let result = builder.call_as(convention, ty, callee, &arguments, "");
    for (at, list) in attrs.into_iter().enumerate() {
        for attr in list {
            builder.argument_attr(at, attr);
        }
    }
    builder.ret(result);
    let GlobalKind::Function(body) = &mut module.globals[id.0 as usize].kind else { unreachable!("a function") };
    let call = body.iter_block(entry).next().expect("the call");
    let Opcode::Call(info) = body.instruction(call).opcode.clone() else { unreachable!("a call") };
    let mut info = info;
    info.tail = Tail::Tail;
    body.set_opcode(call, Opcode::Call(info));
}

#[cfg(test)]
#[path = "mergefunc_tests.rs"]
mod tests;
