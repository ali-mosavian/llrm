//! `X86DAGToDAGISel` as the interpreter's [`Hooks`]: complex patterns, node
//! transforms and the profitability and legality of folding a node into its
//! user.
//!
//! # How it works
//! The generated table names its complex patterns by number; [`X86Hooks`] reads
//! the function each number stands for from the CPU's `COMPLEX_PATTERNS` and
//! runs the matcher of that name from [`crate::x86_addr`]. `selectTLSADDRAddr`
//! and `selectMOV64Imm32` never match here (SD35).
//!
//! Numbered deviations from the pin (reason / owner / exit / review):
//! - **SD36** `checkTCRetEnoughRegs` is `false` and `isDef32`,
//!   `isSExtAbsoluteSymbolRef` are `false`: tail calls, 64-bit zero-extension
//!   and absolute symbols are not translated yet, and `false` only declines a
//!   pattern. Owner sdag-c. Exit: C3. Review 2026-12-10.
//! - **SD221** (m16) `shouldAvoidImmediateInstFormsForSize` does not count a
//!   call argument's store as a use: it is a `push imm` at real mode, smaller
//!   than the register that would hold the constant. The pin has no 16-bit
//!   target to answer for. Owner w12. Exit: a 16-bit size model in the hook.
//!   Review 2026-12-10.
//! - **SD38** `isUnneededShiftMask` reads the mask only, not
//!   `computeKnownBits`: the DAG API has no known-bits yet. Owner sdag-c. Exit:
//!   when it has. Review 2026-12-10.
//!
//! # Used by
//! The shadow harness: `Selector::select(dag, &mut X86Hooks::new(..), node)`.

use crate::dag::{Hooks, PatternContext, SDValue};
use crate::generated::ids::{isd, mvt, x86, x86isd};
use crate::generated::{x86_i486, x86_m16, x86_p5, x86_x64};
use crate::instr;
use crate::matcher::Vt;
use crate::x86_addr::{AddrDag, AddrMode, Matcher, vt_bits};

/// The CPU whose table is walked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cpu {
    I486,
    P5,
    /// Real mode: the pin's table with `m16.td` on top (C2).
    M16,
    /// x86-64: the pin's table in 64-bit mode (D64-0). Pointers are `i64`.
    X64,
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub cpu: Cpu,
    /// `CodeGenOptLevel::None`: nothing is folded.
    pub opt_none: bool,
    /// `shouldOptForSize(MF)`.
    pub opt_for_size: bool,
    /// `MF->getFunction().hasMinSize()`.
    pub min_size: bool,
    /// `Subtarget->getFrameLowering()->hasFP(*MF)`.
    pub has_frame_pointer: bool,
    /// The address forms the target has (SD3).
    pub address_mode: AddrMode,
}

impl Options {
    /// `TLI->getPointerTy`: a word near pointer in real mode.
    pub fn pointer_type(&self) -> Vt {
        if self.address_mode == AddrMode::Real16 {
            mvt::i16
        } else if self.cpu == Cpu::X64 {
            mvt::i64
        } else {
            mvt::i32
        }
    }
}

pub struct X86Hooks {
    pub options: Options,
}

impl X86Hooks {
    /// `Subtarget->slowIncDec()`: only Atom and later cores.
    pub fn slow_inc_dec(&self) -> bool {
        false
    }
}

impl PatternContext for X86Hooks {
    // DEVIATE (m16): the pin's `OptForSize, Not64BitMode` patterns make 1, -1
    // and word constants by `xor; inc`/`dec` or a dword `mov`, which are
    // shorter than `mov r32, imm32` only in 32-bit code: with the 0x66 prefix
    // at -m16 they are longer than `mov ax, imm16`.
    fn should_opt_for_size(&self) -> bool {
        self.options.opt_for_size && self.options.cpu != Cpu::M16
    }
    fn has_min_size(&self) -> bool {
        self.options.min_size
    }
    fn has_frame_pointer(&self) -> bool {
        self.options.has_frame_pointer
    }
    fn is_legal_to_call_immediate_addr(&self) -> bool {
        // Static relocation model, flat, not Win32 or PIC.
        true
    }
}

/// The selector function the table's complex pattern `id` calls.
fn complex_function(
    cpu: Cpu,
    id: u8,
) -> &'static str {
    let patterns = match cpu {
        Cpu::I486 => x86_i486::COMPLEX_PATTERNS,
        Cpu::P5 => x86_p5::COMPLEX_PATTERNS,
        Cpu::M16 => x86_m16::COMPLEX_PATTERNS,
        Cpu::X64 => x86_x64::COMPLEX_PATTERNS,
    };
    patterns.iter().find(|(n, _, _)| *n == id).map_or("", |(_, name, _)| name)
}

fn trailing_ones(
    value: u64,
    width: u32,
) -> u32 {
    (value.trailing_ones()).min(width)
}

fn bits(
    dag: &impl AddrDag,
    v: SDValue,
) -> u32 {
    let vt = dag.value_type(v);
    if vt == mvt::i8 {
        8
    } else if vt == mvt::i16 {
        16
    } else if vt == mvt::i32 {
        32
    } else {
        64
    }
}

impl X86Hooks {
    pub fn new(options: Options) -> X86Hooks {
        X86Hooks { options }
    }

    /// `X86DAGToDAGISel::IsProfitableToFold` ([L692][prof]).
    ///
    /// [prof]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L692
    pub fn is_profitable_to_fold<D: AddrDag>(
        &self,
        dag: &D,
        n: SDValue,
        user: u32,
        root: u32,
    ) -> bool {
        if self.options.opt_none || !dag.has_one_use(n) {
            return false;
        }
        if dag.opcode(n.node) != isd::LOAD {
            return true;
        }
        if user != root {
            return true;
        }
        let constant = |v: SDValue| (dag.opcode(v.node) == isd::Constant).then(|| dag.constant(v)).flatten();
        let one_shl = |v: SDValue| dag.opcode(v.node) == isd::SHL && constant(dag.operand(v.node, 0)) == Some(1);
        let opcode = dag.opcode(user);
        match opcode {
            x86isd::ADD
            | x86isd::ADC
            | x86isd::SUB
            | x86isd::SBB
            | x86isd::AND
            | x86isd::XOR
            | x86isd::OR
            | isd::ADD
            | isd::UADDO_CARRY
            | isd::AND
            | isd::OR
            | isd::XOR => {
                let op1 = dag.operand(user, 1);
                if let Some(imm) = constant(op1) {
                    // A small immediate folds shorter than a load.
                    if i8::try_from(imm).is_ok() {
                        return false;
                    }
                    if opcode == isd::AND && matches!(imm as u64 & 0xffff_ffff, 0xff | 0xffff | 0xffff_ffff) {
                        return false;
                    }
                    if (opcode == isd::ADD || opcode == isd::SUB) && i8::try_from(imm.wrapping_neg()).is_ok() {
                        return false;
                    }
                    if (opcode == x86isd::ADD || opcode == x86isd::SUB)
                        && i8::try_from(imm.wrapping_neg()).is_ok()
                        && self.has_no_carry_flag_uses(dag, SDValue::new(user, 1))
                    {
                        return false;
                    }
                }
                if opcode == isd::OR || opcode == isd::XOR {
                    // `bts`/`btc` take the register form.
                    if one_shl(dag.operand(user, 0)) || one_shl(dag.operand(user, 1)) {
                        return false;
                    }
                }
                if opcode == isd::AND {
                    let rotl_minus_two =
                        |v: SDValue| dag.opcode(v.node) == isd::ROTL && constant(dag.operand(v.node, 0)) == Some(-2);
                    if rotl_minus_two(dag.operand(user, 0)) || rotl_minus_two(dag.operand(user, 1)) {
                        return false;
                    }
                }
            }
            isd::SHL | isd::SRA | isd::SRL if constant(dag.operand(user, 1)).is_some() => return false,
            _ => {}
        }
        true
    }

    /// `SelectionDAGISel::IsLegalToFold` ([L2426][legal]): folding `n` into
    /// `user` under `root` must not make a path from `n` to the root that
    /// goes around `user`.
    ///
    /// [legal]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L2426
    pub fn is_legal_to_fold<D: AddrDag>(
        &self,
        dag: &D,
        n: SDValue,
        user: u32,
        root: u32,
    ) -> bool {
        if self.options.opt_none {
            return false;
        }
        let mut root = root;
        // The root is glued to its user: the user is then the root of the
        // matched code.
        let mut ignore_chains = true;
        while dag.value_type(SDValue::new(root, (dag.num_values(root) - 1) as u8)) == mvt::Glue {
            let Some(glued) = dag.glued_user(root) else { break };
            root = glued;
            ignore_chains = false;
        }
        !find_non_imm_use(dag, root, n.node, user, ignore_chains)
    }

    /// `tryFoldLoad` ([L3483][fold]): the memory operand of `n` when it is a
    /// plain load worth folding into `parent` under `root`.
    ///
    /// [fold]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3483
    pub fn try_fold_load<D: AddrDag>(
        &self,
        dag: &mut D,
        root: u32,
        parent: u32,
        n: SDValue,
    ) -> Option<crate::x86_addr::AddressOperands> {
        let is_non_ext_load =
            dag.opcode(n.node) == isd::LOAD && dag.load_ext_type(n.node) == Some(crate::dag::LoadExt::NonExt);
        if !is_non_ext_load
            || !self.is_profitable_to_fold(dag, n, parent, root)
            || !self.is_legal_to_fold(dag, n, parent, root)
        {
            return None;
        }
        let ptr = dag.operand(n.node, 1);
        Matcher {
            dag,
            pointer_type: self.options.pointer_type(),
            mode: self.options.address_mode,
            size: self.options.opt_for_size,
        }
        .select_addr_of(Some(n.node), ptr)
    }

    /// `hasNoCarryFlagUses` ([L3650][carry]).
    ///
    /// [carry]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3650
    pub fn has_no_carry_flag_uses<D: AddrDag>(
        &self,
        dag: &D,
        flags: SDValue,
    ) -> bool {
        // COND_O..COND_G: the codes that read no carry.
        has_no_flag_uses(dag, flags, |cc| !matches!(cc, 0 | 1 | 4 | 5 | 8..=15))
    }

    /// `hasNoSignFlagUses` ([L3636][sign]). Unlike the pin, a user that reads
    /// the flags directly (`SETCC`, `CMOV`, `BRCOND`) is examined too.
    ///
    /// [sign]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3636
    pub fn has_no_sign_flag_uses<D: AddrDag>(
        &self,
        dag: &D,
        flags: SDValue,
    ) -> bool {
        // COND_S, COND_NS, COND_L, COND_GE, COND_LE, COND_G read the sign.
        has_no_flag_uses(dag, flags, |cc| matches!(cc, 8 | 9 | 12..=15))
    }
}

/// True when no user of `flags` has a condition code for which `reads` holds.
fn has_no_flag_uses<D: AddrDag>(
    dag: &D,
    flags: SDValue,
    reads: impl Fn(u64) -> bool,
) -> bool {
    for (user, _) in dag.users(flags) {
        if dag.opcode(user) == isd::CopyToReg {
            // Only a copy to EFLAGS; the machine nodes that read it say
            // their condition in the last use operand.
            if dag.register_of(dag.operand(user, 1).node) != Some(x86::EFLAGS) {
                return false;
            }
            for (reader, _) in dag.users(SDValue::new(user, 1)) {
                if !dag.is_machine(reader) {
                    return false;
                }
                match machine_condition(dag, reader) {
                    Some(cc) if !reads(cc) => {}
                    _ => return false,
                }
            }
            continue;
        }
        let cc_operand = match dag.opcode(user) {
            x86isd::SETCC | x86isd::SETCC_CARRY => 0,
            x86isd::CMOV | x86isd::BRCOND => 2,
            _ => return false,
        };
        match dag.constant(dag.operand(user, cc_operand)) {
            Some(cc) if !reads(cc as u64) => {}
            _ => return false,
        }
    }
    true
}

/// `getCondFromNode`: the condition a machine node reads, from its last use
/// operand; `None` for an instruction that has none (`COND_INVALID`).
fn machine_condition<D: AddrDag>(
    dag: &D,
    node: u32,
) -> Option<u64> {
    let desc = instr::desc(!dag.opcode(node))?;
    let last = desc.operands.last()?;
    if desc.is_pseudo() || last.kind != instr::operand_type::X86_COND_CODE {
        return None;
    }
    let at = usize::from(desc.num_operands - desc.num_defs) - 1;
    dag.constant(dag.operand(node, at)).map(|cc| cc as u64)
}

/// `findNonImmUse`: `def` is reachable from `root` by a path that avoids
/// `immediate_use`'s edge.
fn find_non_imm_use<D: AddrDag>(
    dag: &D,
    root: u32,
    def: u32,
    immediate_use: u32,
    ignore_chains: bool,
) -> bool {
    if dag.is_only_user_of(immediate_use, def) {
        return false;
    }
    let mut visited = crate::dag::Visited::of(immediate_use);
    let mut worklist = Vec::new();
    let seed = |node: u32, visited: &mut crate::dag::Visited, worklist: &mut Vec<u32>| {
        for i in 0..dag.num_operands(node) {
            let op = dag.operand(node, i);
            if (dag.value_type(op) == mvt::Other && ignore_chains) || op.node == def || !visited.insert(op.node) {
                continue;
            }
            worklist.push(op.node);
        }
    };
    seed(immediate_use, &mut visited, &mut worklist);
    if root != immediate_use {
        seed(root, &mut visited, &mut worklist);
    }
    dag.has_predecessor_helper(def, &mut visited, &mut worklist, 0)
}

impl<D: AddrDag> Hooks<D> for X86Hooks {
    fn pointer_type(&self) -> Vt {
        self.options.pointer_type()
    }

    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool {
        match self.options.cpu {
            Cpu::I486 => x86_i486::pattern_predicate(self, predicate),
            Cpu::P5 => x86_p5::pattern_predicate(self, predicate),
            Cpu::M16 => x86_m16::pattern_predicate(self, predicate),
            Cpu::X64 => x86_x64::pattern_predicate(self, predicate),
        }
    }

    fn check_node_predicate(
        &self,
        dag: &D,
        value: SDValue,
        predicate: u16,
    ) -> bool {
        match self.options.cpu {
            Cpu::I486 => x86_i486::node_predicate(dag, self, value, predicate),
            Cpu::P5 => x86_p5::node_predicate(dag, self, value, predicate),
            Cpu::M16 => x86_m16::node_predicate(dag, self, value, predicate),
            Cpu::X64 => x86_x64::node_predicate(dag, self, value, predicate),
        }
    }

    fn check_complex_pattern(
        &mut self,
        dag: &mut D,
        _root: u32,
        parent: Option<u32>,
        value: SDValue,
        pattern: u8,
        results: &mut Vec<(SDValue, Option<u32>)>,
    ) -> bool {
        let mut matcher = Matcher {
            dag,
            pointer_type: self.options.pointer_type(),
            mode: self.options.address_mode,
            size: self.options.opt_for_size,
        };
        let operands = match complex_function(self.options.cpu, pattern) {
            "selectAddr" => matcher.select_addr_of(parent, value),
            "selectLEAAddr" => matcher.select_lea_addr(value),
            "selectLEA64_Addr" => matcher.select_lea64_addr(value),
            "selectRelocImm" => {
                // `Wrapper sym` as an immediate; the truncated form needs
                // absolute symbols (SD36).
                if matcher.dag.opcode(value.node) != x86isd::Wrapper {
                    return false;
                }
                results.push((matcher.dag.operand(value.node, 0), None));
                return true;
            }
            // ELF TLS and 64-bit immediates (SD35).
            _ => None,
        };
        let Some(a) = operands else { return false };
        results.extend([a.base, a.scale, a.index, a.disp, a.segment].map(|v| (v, None)));
        true
    }

    fn run_xform(
        &mut self,
        dag: &mut D,
        value: SDValue,
        xform: u8,
    ) -> SDValue {
        let width = bits(dag, value);
        let constant = dag.constant(value).expect("an xform of a constant") as u64;
        let masked = if width == 64 { constant } else { constant & ((1u64 << width) - 1) };
        // The table numbers its transforms; the pin's C++ call says what each
        // is.
        let table = match self.options.cpu {
            Cpu::I486 => x86_i486::XFORMS,
            Cpu::P5 => x86_p5::XFORMS,
            Cpu::M16 => x86_m16::XFORMS,
            Cpu::X64 => x86_x64::XFORMS,
        };
        let call = table
            .iter()
            .find(|(n, _)| *n == xform)
            .unwrap_or_else(|| panic!("node transform {xform} is not in the table"))
            .1;
        match call {
            c if c.contains("countr_zero()") => dag
                .target_constant(i64::from(if masked == 0 { width } else { masked.trailing_zeros() } as u8), mvt::i64),
            c if c.starts_with("getI32Imm((uint32_t)N->getZExtValue()") => {
                dag.target_constant(i64::from(masked as u32), mvt::i32)
            }
            c if c.contains("countr_one()") => {
                dag.target_constant(i64::from(trailing_ones(masked, width) as u8), mvt::i64)
            }
            "getI8Imm(Imm ^ 0x0f, SDLoc(N))" => dag.target_constant(i64::from(((masked & 0x0f) ^ 0x0f) as u8), mvt::i8),
            "getI8Imm(Imm ^ 0x03, SDLoc(N))" => dag.target_constant(i64::from(((masked & 0x03) ^ 0x03) as u8), mvt::i8),
            // `inv_cond_XFORM`: `GetOppositeBranchCondition` flips the low bit
            // of a condition code (O/NO, B/AE, E/NE, BE/A, S/NS, P/NP, L/GE,
            // LE/G are adjacent pairs).
            c if c.starts_with("CurDAG->getTargetConstant(X86::GetOppositeBranchCondition(CC)") => {
                dag.target_constant(i64::from((masked as u8) ^ 1), mvt::i8)
            }
            other => panic!("node transform `{other}` is not translated"),
        }
    }

    fn is_profitable_to_fold(
        &self,
        dag: &D,
        value: SDValue,
        user: u32,
        root: u32,
    ) -> bool {
        X86Hooks::is_profitable_to_fold(self, dag, value, user, root)
    }

    fn is_legal_to_fold(
        &self,
        dag: &D,
        value: SDValue,
        user: u32,
        root: u32,
    ) -> bool {
        X86Hooks::is_legal_to_fold(self, dag, value, user, root)
    }

    /// `SelectionDAGISel::CheckAndMask`: the mask is the wanted one, or a
    /// subset whose missing bits are known zero in the operand.
    fn check_and_mask(
        &self,
        dag: &D,
        lhs: SDValue,
        rhs: i64,
        desired: i64,
    ) -> bool {
        let width = vt_bits(dag.value_type(lhs));
        let all = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
        let (actual, desired) = (rhs as u64 & all, desired as u64 & all);
        if actual == desired {
            return true;
        }
        if actual & !desired != 0 {
            return false;
        }
        dag.masked_value_is_zero(lhs, desired & !actual)
    }

    fn check_or_mask(
        &self,
        _: &D,
        _: SDValue,
        _: i64,
        _: i64,
    ) -> bool {
        false
    }

    fn has_sse_unaligned_mem(&self) -> bool {
        false
    }

    fn should_avoid_immediate_inst_forms_for_size(
        &self,
        dag: &D,
        node: u32,
    ) -> bool {
        if !self.options.opt_for_size {
            return false;
        }
        let constant = (dag.opcode(node) == isd::Constant).then(|| dag.constant(SDValue::new(node, 0))).flatten();
        let mut use_count = 0;
        let users: Vec<_> = (0..dag.num_values(node)).flat_map(|r| dag.users(SDValue::new(node, r as u8))).collect();
        for (user, operand) in users {
            if use_count >= 2 {
                break;
            }
            // DEVIATE SD221 (m16): the pin has no 16-bit target. A call
            // argument stored at real mode becomes `push imm`, 2 to 3 bytes
            // against the register's `push r` (1 or 2) and the `mov` that
            // makes it, held across the calls in a register the function
            // saves; the store is not a use that argues for the register.
            if self.options.address_mode == AddrMode::Real16 && dag.is_call_argument_store(user) {
                continue;
            }
            // A machine node, or a store of the value: counts.
            if dag.is_machine(user) || (dag.opcode(user) == isd::STORE && operand == 1) {
                use_count += 1;
                continue;
            }
            if dag.num_operands(user) != 2 {
                continue;
            }
            if constant.is_some_and(|c| i8::try_from(c).is_ok()) {
                continue;
            }
            use_count += 1;
        }
        use_count > 1
    }

    fn is_unneeded_shift_mask(
        &self,
        dag: &D,
        node: u32,
        width: u32,
    ) -> bool {
        let mask = dag.constant(dag.operand(node, 1)).expect("an AND with a constant") as u64;
        trailing_ones(mask, 64) >= width
    }

    fn is_sext_absolute_symbol_ref(
        &self,
        _: &D,
        _: u32,
        _: u32,
    ) -> bool {
        false
    }

    fn has_no_carry_flag_uses(
        &self,
        dag: &D,
        flags: SDValue,
    ) -> bool {
        X86Hooks::has_no_carry_flag_uses(self, dag, flags)
    }

    fn is_def32(
        &self,
        _: &D,
        _: u32,
    ) -> bool {
        false
    }

    fn check_tc_ret_enough_regs(
        &self,
        _: &D,
        _: u32,
    ) -> bool {
        false
    }

    fn may_load_store(
        &self,
        opcode: u16,
    ) -> (bool, bool) {
        crate::generated::memory_flags(opcode)
    }
}
