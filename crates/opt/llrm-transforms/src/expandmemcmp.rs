//! A `memcmp` or `bcmp` of a constant size becomes loads and compares, as
//! LLVM's `ExpandMemCmp`.
//!
//! # How it works
//! A translation of LLVM
//! [`ExpandMemCmp.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp)
//! at d1106deb71cc. The target's `enable_mem_cmp_expansion` names the load
//! sizes, the most loads and the loads in a block; the size is cut into loads
//! (greedy, or with one overlapping load at the end) and each block loads both
//! sides and compares. A call used only against zero is `xor`/`or` of the load
//! pairs; any other is packed `bswap`ped loads compared unsigned, in a
//! chain of blocks with an early exit to `res_block`. A call whose only user
//! asks for one outcome (`< 0`, `> 0`, ...) is one unsigned compare. Not in
//! `minsize`, nor in a function that is sanitized.
//!
//! # Example
//! ```text
//! %c = call i32 @memcmp(ptr %a, ptr %b, i64 8)      ; x86-64, %c == 0
//! %e = icmp eq i32 %c, 0
//! ```
//! becomes `load i64` of both, `icmp ne`, `zext` to `i32`, and the `icmp eq`.
//!
//! # Deviations
//! - DEVIATE E1 (exit 2026-12-31, pin permalink
//!   [L778](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L778)):
//!   the pin's `int` is 32 bits. Here it is the call's own type, and a two-byte
//!   compare in a 16-bit `int` cannot be a subtraction (it wraps), so it takes
//!   the `ucmp` path. Exit: a pin with a 16-bit `int`.
//! - DEVIATE E2 (exit 2026-12-31, pin permalink
//!   [L414](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L414)):
//!   a load at an offset from a constant global is not folded here (the pin's
//!   offset is a constant expression); the `getelementptr` index has the
//!   pointer's index width, not `i64`.
//! - DEVIATE E3 (exit 2026-12-31, pin permalink
//!   [L1061](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L1061)):
//!   `SimplifyInstructionsInBlock` is not run; the combine that follows cleans
//!   the expansion. Dominators are not kept (no `DomTreeUpdater`).
//! - DEVIATE E4: the profile-guided `shouldOptimizeForSize` is false (no
//!   profile here), and `setExplicitlyUnknownBranchWeightsIfProfiled` has no
//!   profile to read.
//!
//! # Parity slot
//! `expandmemcmp@1673` (PassBuilderPipelines.cpp#L1673), between
//! `divrempairs` and `tailrec`.

use llrm_mir::constantfolding::constant_fold_load_from_const_ptr;
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::libfunc::{Generic, TargetLibraryInfo};
use llrm_mir::module::{BlockId, GlobalValue, InstId, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, CallInfo, CastOp, Flags, IntPredicate, Opcode, Tail};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::tti::{MemCmpExpansionOptions, Tti};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::{self, Query};

/// `LoadEntry`: a load of `size` bytes at `offset`.
#[derive(Clone, Copy, Debug)]
struct LoadEntry {
    size: u32,
    offset: u64,
}

/// `commonAlignment(align, offset)`.
fn common_alignment(
    align: u64,
    offset: u64,
) -> u64 {
    if offset == 0 { align } else { align.min(1 << offset.trailing_zeros()) }
}

/// The two pointers' known common alignment, and what the target allows
/// beneath it.
struct Access<'a> {
    tti: &'a dyn Tti,
    common: u64,
    space: u32,
}

impl Access<'_> {
    /// `isAccessAllowed`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L177>
    fn allowed(
        &self,
        load_size: u32,
        offset: u64,
    ) -> bool {
        let align = common_alignment(self.common, offset);
        align >= u64::from(load_size) || self.tti.allows_misaligned_memory_accesses(load_size * 8, self.space, align)
    }
}

/// `computeGreedyLoadSequence`: empty where more than `max_num_loads` are
/// needed.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L202>
fn greedy_load_sequence(
    mut size: u64,
    load_sizes: &[u32],
    max_num_loads: u32,
) -> Vec<LoadEntry> {
    let mut sequence = Vec::new();
    let mut offset = 0;
    for &load_size in load_sizes {
        if size == 0 {
            break;
        }
        let count = size / u64::from(load_size);
        if sequence.len() as u64 + count > u64::from(max_num_loads) {
            return Vec::new();
        }
        for _ in 0..count {
            sequence.push(LoadEntry { size: load_size, offset });
            offset += u64::from(load_size);
        }
        size %= u64::from(load_size);
    }
    sequence
}

/// `computeOverlappingLoadSequence`: whole loads, then one that ends where the
/// data does; empty where that is not wanted or not allowed.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L230>
fn overlapping_load_sequence(
    size: u64,
    max_load_size: u32,
    max_num_loads: u32,
    access: &Access,
) -> Vec<LoadEntry> {
    if size < 2 || max_load_size < 2 {
        return Vec::new();
    }
    let whole = size / u64::from(max_load_size);
    let rest = size - whole * u64::from(max_load_size);
    if rest == 0 || whole + 1 > u64::from(max_num_loads) {
        return Vec::new();
    }
    let mut sequence: Vec<LoadEntry> =
        (0..whole).map(|index| LoadEntry { size: max_load_size, offset: index * u64::from(max_load_size) }).collect();
    let end = whole * u64::from(max_load_size);
    let overlap = end - (u64::from(max_load_size) - rest);
    if !access.allowed(max_load_size, overlap) {
        return Vec::new();
    }
    sequence.push(LoadEntry { size: max_load_size, offset: overlap });
    sequence
}

/// `optimiseLoadSequence`: contiguous tail loads merged into one of an
/// allowed size (three-way compares only).
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L273>
fn optimise_load_sequence(
    sequence: &mut Vec<LoadEntry>,
    options: &MemCmpExpansionOptions,
    max_load_size: u32,
    zero_cmp: bool,
) {
    if zero_cmp || options.allowed_tail_expansions.is_empty() {
        return;
    }
    while sequence.len() >= 2 {
        let last = sequence[sequence.len() - 1];
        let pre_last = sequence[sequence.len() - 2];
        if pre_last.offset + u64::from(pre_last.size) != last.offset {
            break;
        }
        let size = last.size + pre_last.size;
        if !options.allowed_tail_expansions.contains(&size) {
            break;
        }
        // A merged load wider than `max_load_size` is only possible alone.
        if size > max_load_size && sequence.len() > 2 {
            break;
        }
        sequence.truncate(sequence.len() - 2);
        sequence.push(LoadEntry { size, offset: pre_last.offset });
    }
}

/// The decomposition of one call into loads and blocks: `MemCmpExpansion`
/// without the IR.
struct Plan {
    size: u64,
    sequence: Vec<LoadEntry>,
    max_load_size: u32,
    max_block_size: u32,
    loads_per_block: usize,
    max_bytes_per_block: u32,
    zero_cmp: bool,
}

impl Plan {
    /// The `MemCmpExpansion` constructor
    /// ([L321](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L321)):
    /// none where the target's load sizes cannot load `size` bytes.
    fn new(
        size: u64,
        options: &MemCmpExpansionOptions,
        zero_cmp: bool,
        access: &Access,
        max_bytes_per_block: u32,
    ) -> Option<Plan> {
        let skip = options.load_sizes.iter().take_while(|&&load| u64::from(load) > size).count();
        let load_sizes = &options.load_sizes[skip..];
        let max_load_size = *load_sizes.first()?;
        let mut sequence = greedy_load_sequence(size, load_sizes, options.max_num_loads);
        if options.allow_overlapping_loads && (sequence.is_empty() || sequence.len() > 2) {
            let overlapping = overlapping_load_sequence(size, max_load_size, options.max_num_loads, access);
            if !overlapping.is_empty() && (sequence.is_empty() || overlapping.len() < sequence.len()) {
                sequence = overlapping;
            }
        }
        optimise_load_sequence(&mut sequence, options, max_load_size, zero_cmp);
        let mut plan = Plan {
            size,
            sequence,
            max_load_size,
            max_block_size: 0,
            loads_per_block: options.num_loads_per_block as usize,
            max_bytes_per_block,
            zero_cmp,
        };
        let mut index = 0;
        while index < plan.sequence.len() {
            let loads = plan.loads_in_block(index);
            plan.max_block_size = plan.max_block_size.max(plan.bytes_in_block(index, loads));
            index += loads;
        }
        if plan.max_block_size != 0 {
            plan.max_block_size = plan.max_block_size.next_power_of_two();
        }
        Some(plan)
    }

    /// `getNumLoadsInBlock`.
    fn loads_in_block(
        &self,
        index: usize,
    ) -> usize {
        if self.zero_cmp {
            return (self.sequence.len() - index).min(self.loads_per_block);
        }
        let (mut loads, mut bytes) = (0, 0);
        while index + loads < self.sequence.len()
            && loads < self.loads_per_block
            && bytes + self.sequence[index + loads].size <= self.max_bytes_per_block
        {
            bytes += self.sequence[index + loads].size;
            loads += 1;
        }
        assert!(loads > 0, "at least one load must fit in a block");
        loads
    }

    /// `getNumBytesInBlock`.
    fn bytes_in_block(
        &self,
        index: usize,
        loads: usize,
    ) -> u32 {
        self.sequence[index..index + loads].iter().map(|load| load.size).sum()
    }

    /// `getNumBlocks`.
    fn blocks(&self) -> usize {
        let (mut blocks, mut index) = (0, 0);
        while index < self.sequence.len() {
            index += self.loads_in_block(index);
            blocks += 1;
        }
        blocks
    }
}

/// An instruction's arms of a phi: value and the block it comes from.
type Arms = Vec<(Operand, BlockId)>;

/// The `MemCmpExpansion` IR: where the instructions go and the phis' arms.
struct Emitter<'e, 'u> {
    unit: &'e mut Unit<'u>,
    globals: &'e [GlobalValue],
    call: InstId,
    plan: &'e Plan,
    /// The two pointers and their known alignments.
    sources: [(Operand, u64); 2],
    result: TypeId,
    result_bits: u32,
    at: Position,
    blocks: Vec<BlockId>,
    end: Option<BlockId>,
    res_block: Option<BlockId>,
    phi_res: Option<(InstId, Arms)>,
    phi_src: Option<([InstId; 2], [Arms; 2])>,
}

impl Emitter<'_, '_> {
    fn type_of(
        &self,
        operand: Operand,
    ) -> TypeId {
        self.unit.function.operand_type(self.unit.context, operand).expect("a typed operand")
    }

    fn int(
        &self,
        bits: u32,
    ) -> TypeId {
        self.unit.context.types.int(bits)
    }

    fn constant(
        &self,
        ty: TypeId,
        value: i128,
    ) -> Operand {
        Operand::Constant(self.unit.context.int(ty, value))
    }

    fn put_inst(
        &mut self,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
    ) -> InstId {
        let made = self.unit.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        self.unit.function.insert(made, self.at).expect("a placed instruction");
        made
    }

    fn put(
        &mut self,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
    ) -> Operand {
        let made = self.put_inst(opcode, ty, operands);
        Operand::Value(self.unit.function.instruction(made).result.expect("a result"))
    }

    /// `CreateZExt`: the value itself where it already has the type.
    fn zext(
        &mut self,
        value: Operand,
        bits: u32,
    ) -> Operand {
        let ty = self.int(bits);
        if self.type_of(value) == ty { value } else { self.put(Opcode::Cast(CastOp::ZExt), ty, vec![value]) }
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
    ) -> Operand {
        let ty = self.type_of(lhs);
        self.put(Opcode::Binary(op), ty, vec![lhs, rhs])
    }

    fn icmp(
        &mut self,
        predicate: IntPredicate,
        lhs: Operand,
        rhs: Operand,
    ) -> Operand {
        let flag = self.int(1);
        self.put(Opcode::ICmp(predicate), flag, vec![lhs, rhs])
    }

    fn branch(
        &mut self,
        operands: Vec<Operand>,
    ) {
        let void = self.unit.context.types.void();
        self.put_inst(Opcode::Br, void, operands);
    }

    fn call_intrinsic(
        &mut self,
        name: &str,
        returns: TypeId,
        args: &[Operand],
    ) -> Operand {
        let parameters: Vec<TypeId> = args.iter().map(|&arg| self.type_of(arg)).collect();
        let function_type = self.unit.context.types.intern(Type::Function { returns, parameters, variadic: false });
        let callee = self.unit.declared.declare(name, function_type);
        let pointer = self.unit.context.types.ptr(0);
        let callee =
            Operand::Constant(self.unit.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
        let info = CallInfo {
            function_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); args.len()],
            attrs: Vec::new(),
            tail: Tail::default(),
        };
        let mut operands = args.to_vec();
        operands.push(callee);
        self.put(Opcode::Call(Box::new(info)), returns, operands)
    }

    /// `getLoadPair`: both sides' `load_bits`-wide loads at `offset`, byte
    /// swapped in `bswap_bits` and widened to `cmp_bits` where given.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L414>
    fn load_pair(
        &mut self,
        load_bits: u32,
        bswap_bits: Option<u32>,
        cmp_bits: Option<u32>,
        offset: u64,
    ) -> (Operand, Operand) {
        let load_ty = self.int(load_bits);
        let mut pair = [Operand::Constant(self.unit.context.int(load_ty, 0)); 2];
        for (side, slot) in pair.iter_mut().enumerate() {
            let (mut source, mut align) = self.sources[side];
            if offset > 0 {
                let space = self.pointer_space(source);
                let index = self.int(self.unit.layout.pointer(space).index_bits);
                let byte = self.int(8);
                let pointer = self.type_of(source);
                let step = self.constant(index, offset as i128);
                source = self.put(Opcode::GetElementPtr { source: byte }, pointer, vec![source, step]);
                align = common_alignment(align, offset);
            }
            let folded = match source {
                Operand::Constant(id) if offset == 0 => {
                    constant_fold_load_from_const_ptr(self.unit.context, self.unit.layout, self.globals, id, load_ty)
                        .map(Operand::Constant)
                }
                _ => None,
            };
            *slot = folded.unwrap_or_else(|| {
                self.put(Opcode::Load { align: Some(align), volatile: false }, load_ty, vec![source])
            });
        }
        let [mut lhs, mut rhs] = pair;
        if let Some(bits) = bswap_bits {
            if bits != load_bits {
                lhs = self.zext(lhs, bits);
                rhs = self.zext(rhs, bits);
            }
            let ty = self.int(bits);
            let name = format!("llvm.bswap.i{bits}");
            lhs = self.call_intrinsic(&name, ty, &[lhs]);
            rhs = self.call_intrinsic(&name, ty, &[rhs]);
        }
        if let Some(bits) = cmp_bits {
            lhs = self.zext(lhs, bits);
            rhs = self.zext(rhs, bits);
        }
        (lhs, rhs)
    }

    fn pointer_space(
        &self,
        pointer: Operand,
    ) -> u32 {
        match self.unit.context.types.get(self.type_of(pointer)) {
            Type::Pointer(space) => *space,
            _ => 0,
        }
    }

    /// `PowerOf2Ceil(bits)` for the byte swap of a load, none on a big-endian
    /// target or for one byte.
    fn bswap_bits(
        &self,
        load_size: u32,
    ) -> Option<u32> {
        (!self.unit.layout.big_endian && load_size != 1).then(|| (load_size * 8).next_power_of_two())
    }

    /// `createLoadCmpBlocks`, `createResultBlock`, `setupEndBlockPHINodes`,
    /// `setupResultBlockPHINodes` and the split, for a plan of several blocks.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L858>
    fn build_frame(&mut self) {
        let start = self.unit.function.parent(self.call).expect("a placed call");
        let end = crate::edges::split_block_at(self.unit.function, start, self.call);
        self.unit.function.set_block_name(end, "endblock");
        self.end = Some(end);
        self.at = Position::Before(self.call);
        let phi = self.put_inst(Opcode::Phi, self.result, Vec::new());
        self.phi_res = Some((phi, Vec::new()));

        let res_block = self.unit.function.create_block(Some("res_block"));
        self.unit.function.insert_block_before(res_block, end).expect("a new block");
        self.res_block = Some(res_block);
        if !self.plan.zero_cmp {
            let ty = self.int(self.plan.max_block_size * 8);
            self.at = Position::End(res_block);
            let first = self.put_inst(Opcode::Phi, ty, Vec::new());
            let second = self.put_inst(Opcode::Phi, ty, Vec::new());
            for (phi, name) in [(first, "phi.src1"), (second, "phi.src2")] {
                let value = self.unit.function.instruction(phi).result.expect("a phi");
                self.unit.function.set_name(value, name);
            }
            self.phi_src = Some(([first, second], [Vec::new(), Vec::new()]));
        }
        for _ in 0..self.plan.blocks() {
            let block = self.unit.function.create_block(Some("loadbb"));
            self.unit.function.insert_block_before(block, end).expect("a new block");
            self.blocks.push(block);
        }
        let jump = self.unit.function.terminator(start).expect("a terminated block");
        self.unit.function.set_operand(jump, 0, Operand::Block(self.blocks[0]));
    }

    fn next_block(
        &self,
        index: usize,
    ) -> BlockId {
        self.blocks.get(index + 1).copied().unwrap_or_else(|| self.end.expect("a frame"))
    }

    fn add_res_arm(
        &mut self,
        value: Operand,
        from: BlockId,
    ) {
        self.phi_res.as_mut().expect("a frame").1.push((value, from));
    }

    /// `emitLoadCompareByteBlock`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L470>
    fn emit_load_compare_byte_block(
        &mut self,
        index: usize,
        offset: u64,
    ) {
        let block = self.blocks[index];
        self.at = Position::End(block);
        let (lhs, rhs) = self.load_pair(8, None, Some(self.result_bits), offset);
        let diff = self.binary(BinaryOp::Sub, lhs, rhs);
        self.add_res_arm(diff, block);
        let end = self.end.expect("a frame");
        if index < self.blocks.len() - 1 {
            let zero = self.constant(self.result, 0);
            let differs = self.icmp(IntPredicate::Ne, diff, zero);
            self.branch(vec![differs, Operand::Block(end), Operand::Block(self.blocks[index + 1])]);
        } else {
            self.branch(vec![Operand::Block(end)]);
        }
    }

    /// `getCompareLoadPairs`: whether the loads of the block differ.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L502>
    fn compare_load_pairs(
        &mut self,
        index: usize,
        load_index: &mut usize,
    ) -> Operand {
        let loads = self.plan.loads_in_block(*load_index);
        self.at = match self.blocks.get(index) {
            Some(&block) => Position::End(block),
            None => Position::Before(self.call),
        };
        let max_load_bits = (loads != 1).then(|| self.plan.max_load_size * 8);
        let mut xors = Vec::new();
        let mut single = None;
        for _ in 0..loads {
            let entry = self.plan.sequence[*load_index];
            *load_index += 1;
            let (lhs, rhs) = self.load_pair(entry.size * 8, None, max_load_bits, entry.offset);
            if loads != 1 {
                let diff = self.binary(BinaryOp::Xor, lhs, rhs);
                xors.push(self.zext(diff, max_load_bits.expect("several loads")));
            } else {
                single = Some(self.icmp(IntPredicate::Ne, lhs, rhs));
            }
        }
        if let Some(cmp) = single {
            return cmp;
        }
        // Pairwise OR of the XORs until one is left.
        let mut list = xors;
        while list.len() != 1 {
            let mut merged = Vec::new();
            for pair in list.chunks(2) {
                merged.push(match pair {
                    [first, second] => self.binary(BinaryOp::Or, *first, *second),
                    [last] => *last,
                    _ => unreachable!("chunks of two"),
                });
            }
            list = merged;
        }
        let zero = self.constant(self.type_of(list[0]), 0);
        self.icmp(IntPredicate::Ne, list[0], zero)
    }

    /// `getPackedLoadPair`: the block's loads as one integer each side, the
    /// lowest address most significant.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L570>
    fn packed_load_pair(
        &mut self,
        index: usize,
        load_index: &mut usize,
    ) -> (Operand, Operand) {
        self.at = match self.blocks.get(index) {
            Some(&block) => Position::End(block),
            None => Position::Before(self.call),
        };
        let loads = self.plan.loads_in_block(*load_index);
        let mut remaining = self.plan.bytes_in_block(*load_index, loads);
        let block_bits = self.plan.max_block_size * 8;
        let block_ty = self.int(block_bits);
        let mut packed: [Option<Operand>; 2] = [None, None];
        for _ in 0..loads {
            let entry = self.plan.sequence[*load_index];
            *load_index += 1;
            let bswap = self.bswap_bits(entry.size);
            let (mut lhs, mut rhs) = self.load_pair(entry.size * 8, bswap, Some(block_bits), entry.offset);
            if let Some(bits) = bswap.filter(|&bits| bits != entry.size * 8) {
                // The wide byte swap left the bytes in the high end.
                let padding = self.constant(block_ty, i128::from(bits - entry.size * 8));
                lhs = self.binary(BinaryOp::LShr, lhs, padding);
                rhs = self.binary(BinaryOp::LShr, rhs, padding);
            }
            remaining -= entry.size;
            let shift = remaining * 8;
            for (slot, value) in packed.iter_mut().zip([lhs, rhs]) {
                // `CreateShl` of zero returns the value; `Or` with zero is
                // folded here, as the first term.
                let shifted = if shift == 0 {
                    value
                } else {
                    let amount = self.constant(block_ty, i128::from(shift));
                    self.binary(BinaryOp::Shl, value, amount)
                };
                *slot = Some(match *slot {
                    None => shifted,
                    Some(so_far) => self.binary(BinaryOp::Or, so_far, shifted),
                });
            }
        }
        (packed[0].expect("a load"), packed[1].expect("a load"))
    }

    /// `emitLoadCompareBlockMultipleLoads`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L615>
    fn emit_load_compare_block_multiple_loads(
        &mut self,
        index: usize,
        load_index: &mut usize,
    ) {
        let differs = self.compare_load_pairs(index, load_index);
        let next = self.next_block(index);
        let res_block = self.res_block.expect("a frame");
        self.branch(vec![differs, Operand::Block(res_block), Operand::Block(next)]);
        if index == self.blocks.len() - 1 {
            let zero = self.constant(self.result, 0);
            self.add_res_arm(zero, self.blocks[index]);
        }
    }

    /// `emitLoadCompareBlock`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L650>
    fn emit_load_compare_block(
        &mut self,
        index: usize,
        load_index: &mut usize,
    ) {
        let loads = self.plan.loads_in_block(*load_index);
        if loads == 1 && self.plan.sequence[*load_index].size == 1 {
            let offset = self.plan.sequence[*load_index].offset;
            self.emit_load_compare_byte_block(index, offset);
            *load_index += 1;
            return;
        }
        let block = self.blocks[index];
        let (lhs, rhs) = if loads == 1 {
            let entry = self.plan.sequence[*load_index];
            *load_index += 1;
            self.at = Position::End(block);
            let bswap = self.bswap_bits(entry.size);
            self.load_pair(entry.size * 8, bswap, Some(self.plan.max_block_size * 8), entry.offset)
        } else {
            self.packed_load_pair(index, load_index)
        };
        if let Some((_, arms)) = self.phi_src.as_mut() {
            arms[0].push((lhs, block));
            arms[1].push((rhs, block));
        }
        let equal = self.icmp(IntPredicate::Eq, lhs, rhs);
        let next = self.next_block(index);
        let res_block = self.res_block.expect("a frame");
        self.branch(vec![equal, Operand::Block(next), Operand::Block(res_block)]);
        if index == self.blocks.len() - 1 {
            let zero = self.constant(self.result, 0);
            self.add_res_arm(zero, block);
        }
    }

    /// `emitMemCmpResultBlock`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L708>
    fn emit_result_block(&mut self) {
        let res_block = self.res_block.expect("a frame");
        let end = self.end.expect("a frame");
        self.at = Position::End(res_block);
        if self.plan.zero_cmp {
            let one = self.constant(self.result, 1);
            self.add_res_arm(one, res_block);
        } else {
            let ([first, second], _) = self.phi_src.clone().expect("result phis");
            let value = |inst: InstId, emitter: &Self| {
                Operand::Value(emitter.unit.function.instruction(inst).result.expect("a phi"))
            };
            let (first, second) = (value(first, self), value(second, self));
            let less = self.icmp(IntPredicate::Ult, first, second);
            let minus = self.constant(self.result, -1);
            let one = self.constant(self.result, 1);
            let picked = self.put(Opcode::Select, self.result, vec![less, minus, one]);
            self.add_res_arm(picked, res_block);
        }
        self.branch(vec![Operand::Block(end)]);
    }

    /// `getMemCmpResult`: the ordering of the loaded values, or only the
    /// outcome the call's single user reads. `None`: the call is replaced.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L809>
    fn mem_cmp_result(
        &mut self,
        lhs: Operand,
        rhs: Operand,
    ) -> Option<Operand> {
        let value = self.unit.function.instruction(self.call).result.expect("a result");
        let users: Vec<InstId> = self.unit.function.users(value).into_iter().map(|one| one.user).collect();
        if let [user] = users[..] {
            let inst = self.unit.function.instruction(user);
            let (opcode, operands, user_ty) = (inst.opcode.clone(), inst.operands.to_vec(), inst.ty);
            let call = Operand::Value(value);
            let constant = |operand: Operand| match operand {
                Operand::Constant(id) => match self.unit.context.get(id).kind {
                    ConstantKind::Int(bits) => Some(bits),
                    _ => None,
                },
                _ => None,
            };
            let ones = llrm_mir::context::mask(self.result_bits);
            let mut zext = false;
            let predicate = match opcode {
                Opcode::Binary(BinaryOp::LShr) if constant(operands[1]) == Some(u128::from(self.result_bits - 1)) => {
                    zext = true;
                    Some(IntPredicate::Slt)
                }
                Opcode::ICmp(IntPredicate::Sgt) if operands[0] == call && constant(operands[1]) == Some(ones) => {
                    Some(IntPredicate::Sge)
                }
                Opcode::ICmp(IntPredicate::Slt) if operands[0] == call && constant(operands[1]) == Some(1) => {
                    Some(IntPredicate::Sle)
                }
                Opcode::ICmp(predicate) if operands[0] == call && constant(operands[1]) == Some(0) => Some(predicate),
                _ => None,
            };
            let unsigned = match predicate {
                Some(IntPredicate::Slt) => Some(IntPredicate::Ult),
                Some(IntPredicate::Sle) => Some(IntPredicate::Ule),
                Some(IntPredicate::Sgt) => Some(IntPredicate::Ugt),
                Some(IntPredicate::Sge) => Some(IntPredicate::Uge),
                _ => None,
            };
            if let Some(unsigned) = unsigned {
                let cmp = self.icmp(unsigned, lhs, rhs);
                let result = if zext { self.put(Opcode::Cast(CastOp::ZExt), user_ty, vec![cmp]) } else { cmp };
                let user_value = self.unit.function.instruction(user).result.expect("a result");
                self.unit.function.replace_value(user_value, result);
                self.unit.function.erase(user).expect("replaced");
                self.unit.function.erase(self.call).expect("its user is gone");
                return None;
            }
        }
        let arg = self.type_of(lhs);
        let bits = self.unit.context.types.int_bits(arg).expect("an integer");
        let name = format!("llvm.ucmp.i{}.i{bits}", self.result_bits);
        Some(self.call_intrinsic(&name, self.result, &[lhs, rhs]))
    }

    /// `getMemCmpOneBlock`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L778>
    fn one_block(&mut self) -> Option<Operand> {
        let size = self.plan.size;
        let bswap = self.bswap_bits(size as u32);
        let load_bits = size as u32 * 8;
        // DEVIATE E1: a two-byte subtraction needs a result wider than 16.
        if (size == 1 || size == 2) && self.result_bits > load_bits {
            let (lhs, rhs) = self.load_pair(load_bits, bswap, Some(self.result_bits), 0);
            return Some(self.binary(BinaryOp::Sub, lhs, rhs));
        }
        let max_bits = self.plan.max_load_size.max((size as u32).next_power_of_two()) * 8;
        let (lhs, rhs) = self.load_pair(load_bits, bswap, Some(max_bits), 0);
        self.mem_cmp_result(lhs, rhs)
    }

    /// `getMemCmpExpansion`: the value of the call, `None` where it is
    /// replaced already.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L858>
    fn expand(&mut self) -> Option<Operand> {
        let blocks = self.plan.blocks();
        if blocks != 1 {
            self.build_frame();
        } else {
            self.at = Position::Before(self.call);
        }
        let mut load_index = 0;
        if self.plan.zero_cmp {
            if blocks == 1 {
                let differs = self.compare_load_pairs(0, &mut load_index);
                return Some(self.zext(differs, self.result_bits));
            }
            for index in 0..blocks {
                self.emit_load_compare_block_multiple_loads(index, &mut load_index);
            }
            self.emit_result_block();
            return Some(self.finish());
        }
        if blocks == 1 {
            self.at = Position::Before(self.call);
            if self.plan.sequence.len() == 1 {
                return self.one_block();
            }
            let (lhs, rhs) = self.packed_load_pair(0, &mut load_index);
            return self.mem_cmp_result(lhs, rhs);
        }
        for index in 0..blocks {
            self.emit_load_compare_block(index, &mut load_index);
        }
        self.emit_result_block();
        Some(self.finish())
    }

    /// The phis' arms, set; the value of `phi.res`.
    fn finish(&mut self) -> Operand {
        let flat = |arms: &Arms| -> Vec<Operand> {
            arms.iter().flat_map(|&(value, from)| [value, Operand::Block(from)]).collect()
        };
        if let Some(([first, second], [lhs, rhs])) = self.phi_src.take() {
            self.unit.function.set_operands(first, flat(&lhs));
            self.unit.function.set_operands(second, flat(&rhs));
        }
        let (phi, arms) = self.phi_res.take().expect("a frame");
        self.unit.function.set_operands(phi, flat(&arms));
        Operand::Value(self.unit.function.instruction(phi).result.expect("a phi"))
    }
}

/// `getMemCmpArgAlignment`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L62>
fn arg_alignment(
    unit: &Unit,
    globals: &[GlobalValue],
    call: InstId,
    arg: usize,
) -> u64 {
    let instruction = unit.function.instruction(call);
    let known = valuetracking::alignment(unit.context, unit.layout, globals, unit.function, instruction.operands[arg]);
    let stated = match &instruction.opcode {
        Opcode::Call(info) => info.argument_attrs.get(arg).and_then(|attrs| {
            attrs
                .iter()
                .find_map(
                    |one| match one {
                        Attribute::Int(name, bytes) if name == "align" => Some(*bytes),
                        _ => None,
                    },
                )
        }),
        _ => None,
    };
    known.max(stated.unwrap_or(1))
}

/// `expandMemCmp`: whether the call was expanded.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L976>
fn expand_mem_cmp(
    unit: &mut Unit,
    globals: &[GlobalValue],
    tti: &dyn Tti,
    call: InstId,
    is_bcmp: bool,
) -> bool {
    if unit.function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(name) if name == "minsize")) {
        return false;
    }
    let operands = unit.function.instruction(call).operands.to_vec();
    let Operand::Constant(size) = operands[2] else { return false };
    let ConstantKind::Int(size) = unit.context.get(size).kind else { return false };
    let size = size as u64;
    if size == 0 {
        return false;
    }
    let zero_cmp = is_bcmp
        || valuetracking::known::is_only_used_in_zero_equality_comparison(
            &Query::bare(unit.context, unit.function, unit.layout),
            call,
        );
    // DEVIATE E4: no profile, so the block is never cold.
    let Some(mut options) = tti.enable_mem_cmp_expansion(false, zero_cmp) else { return false };
    if options.load_sizes.is_empty() {
        return false;
    }
    let space = match unit.context.types.get(unit.function.operand_type(unit.context, operands[0]).expect("typed")) {
        Type::Pointer(space) => *space,
        _ => 0,
    };
    let aligns = [arg_alignment(unit, globals, call, 0), arg_alignment(unit, globals, call, 1)];
    let access = Access { tti, common: aligns[0].min(aligns[1]), space };
    let max_bytes_per_block = options.load_sizes[0];
    options.load_sizes.retain(|&load_size| access.allowed(load_size, 0));
    if options.load_sizes.is_empty() {
        return false;
    }
    let Some(plan) = Plan::new(size, &options, zero_cmp, &access, max_bytes_per_block) else { return false };
    if plan.sequence.is_empty() {
        return false;
    }
    let result = unit.function.instruction(call).ty;
    let result_bits = unit.context.types.int_bits(result).expect("an integer result");
    let value = unit.function.instruction(call).result.expect("a result");
    let mut emitter = Emitter {
        unit,
        globals,
        call,
        plan: &plan,
        sources: [(operands[0], aligns[0]), (operands[1], aligns[1])],
        result,
        result_bits,
        at: Position::Before(call),
        blocks: Vec::new(),
        end: None,
        res_block: None,
        phi_res: None,
        phi_src: None,
    };
    if let Some(replacement) = emitter.expand() {
        unit.function.replace_value(value, replacement);
        unit.function.erase(call).expect("replaced");
    }
    true
}

pub struct ExpandMemCmp;

impl FunctionPass for ExpandMemCmp {
    fn adds_memory_operations(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "expandmemcmp"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("expand-memcmp")
    }

    /// `ExpandMemCmpPass::run`
    /// ([L1098](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ExpandMemCmp.cpp#L1098))
    /// and `runImpl` (L1061).
    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let sanitized = ["sanitize_address", "sanitize_memory", "sanitize_thread", "sanitize_hwaddress"];
        if unit.function.attrs.iter().any(|attr| {
            matches!(
                attr,
                Attribute::Flag(name) if sanitized.contains(&name.as_str())
            )
        }) {
            return PreservedAnalyses::all();
        }
        let outer = analyses.outer().clone();
        let Some(tti) = outer.target().tti() else { return PreservedAnalyses::all() };
        let size_bits = unit.layout.pointer(0).index_bits;
        let info = outer.memo::<TargetLibraryInfo>(0, || {
            let library =
                outer.program().target.library().unwrap_or_else(|| std::rc::Rc::new(Generic::for_pointer(size_bits)));
            TargetLibraryInfo::new(&*library)
        });
        let mut calls = Vec::new();
        for block in unit.function.layout().to_vec() {
            for inst in unit.function.iter_block(block).collect::<Vec<_>>() {
                if !matches!(unit.function.instruction(inst).opcode, Opcode::Call(_)) {
                    continue;
                }
                let Some(global) = llrm_mir::memory::callee(unit.context, unit.function, inst) else { continue };
                let func =
                    outer.globals.get(global.0 as usize).and_then(|one| info.get_lib_func_of(&unit.context.types, one));
                match func {
                    Some(llrm_mir::libfunc::LibFunc::Memcmp) => calls.push((inst, false)),
                    Some(llrm_mir::libfunc::LibFunc::Bcmp) => calls.push((inst, true)),
                    _ => {}
                }
            }
        }
        let mut changed = false;
        for (call, is_bcmp) in calls {
            changed |= expand_mem_cmp(unit, &outer.globals, &*tti, call, is_bcmp);
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

#[cfg(test)]
#[path = "expandmemcmp_tests.rs"]
mod tests;
