//! `M68kISelAddressMode` and the `Select*` complex patterns of
//! `M68kDAGToDAGISel`.
//!
//! # How it works
//! `matchAddress` folds a pointer expression into one [`AddressMode`]: a base
//! register or frame index, an optional index register, a displacement and a
//! symbol. Each `Select*` then asks for its own shape (`ARI` a bare base,
//! `ARID` base and displacement, `ARII` base and index, `AL` a
//! symbol or absolute, `PCD` and `PCI` the PC-relative forms) and returns the
//! operands the table expects, in the order of the pin's signature.
//!
//! DEVIATE: `SelectARIPI` and `SelectARIPD` answer false, as the pin's do ("NOT
//! IMPLEMENTED"). `SelectInlineAsmMemoryOperand` is not translated: no inline
//! asm in the slice.
//!
//! # See also
//! `llvm/lib/Target/M68k/M68kISelDAGToDAG.cpp`, pin d1106deb71cc.

use llrm_dagisel::dag::SDValue;
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::x86_addr::{AddrDag, Symbol, SymbolKind};

use super::generated::ids::{generic, isd as m68k_isd, reg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddrType {
    /// Address register indirect.
    Ari,
    /// Address register indirect with displacement.
    Arid,
    /// Address register indirect with index.
    Arii,
    /// PC with displacement.
    Pcd,
    /// PC with index.
    Pci,
    /// Absolute.
    Al,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Base {
    Reg,
    FrameIndex,
}

#[derive(Clone, Copy, Debug)]
pub struct AddressMode {
    am: AddrType,
    base_type: Base,
    disp: i64,
    base_reg: Option<SDValue>,
    base_frame_index: i32,
    index_reg: Option<SDValue>,
    symbol: Option<Symbol>,
    symbol_flags: u8,
}

/// The pointer type: `MVT::i32` (the `TLI->getPointerTy`).
const POINTER: llrm_dagisel::matcher::Vt = mvt::i32;

impl AddressMode {
    fn new(am: AddrType) -> Self {
        AddressMode {
            am,
            base_type: Base::Reg,
            disp: 0,
            base_reg: None,
            base_frame_index: 0,
            index_reg: None,
            symbol: None,
            symbol_flags: 0,
        }
    }

    fn has_symbolic_displacement(&self) -> bool {
        self.symbol.is_some()
    }

    fn has_base(&self) -> bool {
        self.base_type == Base::FrameIndex || self.base_reg.is_some()
    }

    fn has_base_reg(&self) -> bool {
        self.base_type == Base::Reg && self.base_reg.is_some()
    }

    fn has_index_reg(&self) -> bool {
        self.base_type == Base::Reg && self.index_reg.is_some()
    }

    fn is_disp_addr_type(&self) -> bool {
        matches!(
            self.am,
            AddrType::Arii | AddrType::Pci | AddrType::Arid | AddrType::Pcd | AddrType::Al
        )
    }

    fn disp_size(&self) -> u32 {
        match self.am {
            AddrType::Arii | AddrType::Pci => 8,
            AddrType::Arid | AddrType::Pcd => 16,
            AddrType::Al => 32,
            AddrType::Ari => 0,
        }
    }

    fn is_disp32(&self) -> bool {
        self.disp_size() == 32
    }

    fn is_pc_relative<D: AddrDag>(
        &self,
        dag: &D,
    ) -> bool {
        self.base_type == Base::Reg && self.base_reg.is_some_and(|base| dag.register_of(base.node) == Some(reg::PC))
    }
}

/// `isIntN(bits, value)`.
fn is_int_n(
    bits: u32,
    value: i64,
) -> bool {
    bits >= 64 || (-(1i64 << (bits - 1))..(1i64 << (bits - 1))).contains(&value)
}

/// `doesDispFitFI`: one bit less, so a resolved frame index still fits.
fn does_disp_fit_fi(am: &AddressMode) -> bool {
    am.is_disp_addr_type() && is_int_n(am.disp_size() - 1, am.disp)
}

fn does_disp_fit(
    am: &AddressMode,
    value: i64,
) -> bool {
    am.is_disp_addr_type() && is_int_n(am.disp_size(), value)
}

fn fold_offset_into_address(
    offset: i64,
    am: &mut AddressMode,
) -> bool {
    // Cannot combine external-symbol displacements with integer offsets.
    if offset != 0 && am.symbol.is_some_and(|s| matches!(s.kind, SymbolKind::External | SymbolKind::McSymbol)) {
        return false;
    }
    let value = am.disp + offset;
    if does_disp_fit(am, value) {
        am.disp = value;
        return true;
    }
    false
}

/// `matchAddressBase`.
fn match_address_base(
    n: SDValue,
    am: &mut AddressMode,
) -> bool {
    if am.has_base() {
        // DEVIATE from the pin, which sets an index beside a frame index base
        // that `getFrameIndexAddress` then drops: `a[i]` of a stack array is
        // stored to `a[0]` (the pin's `llc` does the same, at every level).
        // The frame index goes in a register instead. Exit: never; a fix the
        // pin lacks.
        if am.base_type == Base::FrameIndex {
            return false;
        }
        if !am.has_index_reg() {
            am.index_reg = Some(n);
            return true;
        }
        return false;
    }
    am.base_type = Base::Reg;
    am.base_reg = Some(n);
    true
}

/// `matchWrapper`: a `Wrapper` or `WrapperPC` of a symbol.
fn match_wrapper<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    am: &mut AddressMode,
) -> bool {
    // A symbol is already the displacement: never match another.
    if am.has_symbolic_displacement() {
        return false;
    }
    let Some(wrapped) = dag.wrapped_symbol(dag.operand(n.node, 0)) else { return false };
    let offset_counts =
        !matches!(
            wrapped.symbol.kind,
            SymbolKind::External | SymbolKind::McSymbol | SymbolKind::JumpTable
        );
    if dag.opcode(n.node) == m68k_isd::WrapperPC {
        let backup = *am;
        if am.has_base() {
            return false;
        }
        am.symbol = Some(wrapped.symbol);
        // MCSymbol sets no flags in the pin.
        if wrapped.symbol.kind != SymbolKind::McSymbol {
            am.symbol_flags = wrapped.flags;
        }
        if offset_counts && !fold_offset_into_address(wrapped.offset, am) {
            *am = backup;
            return false;
        }
        am.base_type = Base::Reg;
        am.base_reg = Some(dag.register(reg::PC, mvt::i32));
        return true;
    }
    // This wrapper needs a 32-bit displacement.
    if !am.is_disp32() {
        return false;
    }
    if dag.opcode(n.node) == m68k_isd::Wrapper {
        am.symbol = Some(wrapped.symbol);
        if wrapped.symbol.kind != SymbolKind::McSymbol {
            am.symbol_flags = wrapped.flags;
        }
        if offset_counts {
            am.disp += wrapped.offset;
        }
        return true;
    }
    false
}

/// `matchADD`: both operands into the address, then commuted, then as base and
/// index.
fn match_add<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    am: &mut AddressMode,
    depth: u32,
) -> bool {
    let (lhs, rhs) = (dag.operand(n.node, 0), dag.operand(n.node, 1));
    let backup = *am;
    if match_address_recursively(dag, lhs, am, depth + 1) && match_address_recursively(dag, rhs, am, depth + 1) {
        return true;
    }
    *am = backup;
    if match_address_recursively(dag, rhs, am, depth + 1) && match_address_recursively(dag, lhs, am, depth + 1) {
        return true;
    }
    *am = backup;
    // Put each operand in a register and fold at least the add.
    if !am.has_base() && !am.has_index_reg() {
        am.base_reg = Some(lhs);
        am.index_reg = Some(rhs);
        return true;
    }
    false
}

/// `matchAddressRecursively`.
fn match_address_recursively<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    am: &mut AddressMode,
    depth: u32,
) -> bool {
    if depth > 5 {
        return match_address_base(n, am);
    }
    // A PC-relative address takes only immediates.
    if am.is_pc_relative(dag) {
        return dag.opcode(n.node) == isd::Constant
            && dag.constant(n).is_some_and(|value| fold_offset_into_address(value, am));
    }
    let opcode = dag.opcode(n.node);
    if opcode == isd::Constant {
        if dag.constant(n).is_some_and(|value| fold_offset_into_address(value, am)) {
            return true;
        }
    } else if opcode == m68k_isd::Wrapper || opcode == m68k_isd::WrapperPC {
        if match_wrapper(dag, n, am) {
            return true;
        }
    } else if opcode == isd::OR {
        // An `or` of disjoint bits is an `add`.
        if dag.have_no_common_bits_set(dag.operand(n.node, 0), dag.operand(n.node, 1)) && match_add(dag, n, am, depth) {
            return true;
        }
    } else if opcode == isd::ADD {
        if match_add(dag, n, am, depth) {
            return true;
        }
    } else if opcode == isd::FrameIndex {
        if am.is_disp_addr_type() && am.base_type == Base::Reg && am.base_reg.is_none() && does_disp_fit_fi(am) {
            if let Some(index) = dag.frame_index(n) {
                am.base_type = Base::FrameIndex;
                am.base_frame_index = index;
                return true;
            }
        }
    } else if opcode == isd::TargetGlobalTLSAddress {
        if let Some(wrapped) = dag.wrapped_symbol(n) {
            am.symbol = Some(wrapped.symbol);
            am.symbol_flags = wrapped.flags;
            return true;
        }
    }
    match_address_base(n, am)
}

fn match_address<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
    am: &mut AddressMode,
) -> bool {
    match_address_recursively(dag, n, am, 0)
}

fn i8_imm<D: AddrDag>(
    dag: &mut D,
    value: i64,
) -> SDValue {
    dag.target_constant(value, mvt::i8)
}

fn i16_imm<D: AddrDag>(
    dag: &mut D,
    value: i64,
) -> SDValue {
    dag.target_constant(value, mvt::i16)
}

fn i32_imm<D: AddrDag>(
    dag: &mut D,
    value: i64,
) -> SDValue {
    dag.target_constant(value, mvt::i32)
}

/// `getFrameIndexAddress`: the frame index as base, its displacement as an
/// `i32`.
fn frame_index_address<D: AddrDag>(
    dag: &mut D,
    am: &AddressMode,
) -> Option<(SDValue, SDValue)> {
    if am.base_type != Base::FrameIndex {
        return None;
    }
    let disp = i32_imm(dag, am.disp);
    let base = dag.target_frame_index(am.base_frame_index, POINTER);
    Some((disp, base))
}

/// `getSymbolicDisplacement`. A jump table, external or MC symbol takes no
/// displacement.
fn symbolic_displacement<D: AddrDag>(
    dag: &mut D,
    am: &AddressMode,
) -> Option<SDValue> {
    let symbol = am.symbol?;
    let disp = match symbol.kind {
        SymbolKind::External | SymbolKind::McSymbol | SymbolKind::JumpTable => 0,
        _ => am.disp,
    };
    Some(dag.target_symbol(symbol, disp as i32, am.symbol_flags))
}

/// `isAddressBase`: reaches a wrapper or the global base register.
fn is_address_base<D: AddrDag>(
    dag: &D,
    n: SDValue,
) -> bool {
    let opcode = dag.opcode(n.node);
    if opcode == isd::ADD || opcode == isd::ADDC {
        return (0..dag.num_operands(n.node)).any(|i| is_address_base(dag, dag.operand(n.node, i)));
    }
    opcode == m68k_isd::Wrapper || opcode == m68k_isd::WrapperPC || opcode == m68k_isd::GLOBAL_BASE_REG
}

fn allow_arii_with_zero_disp<D: AddrDag>(
    dag: &D,
    parent: Option<u32>,
) -> bool {
    let Some(parent) = parent else { return false };
    let opcode = dag.opcode(parent);
    [isd::LOAD, isd::STORE, isd::ATOMIC_LOAD, isd::ATOMIC_STORE, generic::ATOMIC_CMP_SWAP].contains(&opcode)
}

/// `SelectARI`: `[base]`.
pub fn select_ari<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<SDValue> {
    let mut am = AddressMode::new(AddrType::Ari);
    if !match_address(dag, n, &mut am) || am.is_pc_relative(dag) {
        return None;
    }
    if am.has_index_reg() || am.disp != 0 || am.has_symbolic_displacement() {
        return None;
    }
    am.has_base_reg().then(|| am.base_reg).flatten()
}

/// `SelectARID`: `[disp, base]`.
pub fn select_arid<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<[SDValue; 2]> {
    let mut am = AddressMode::new(AddrType::Arid);
    if !match_address(dag, n, &mut am) || am.is_pc_relative(dag) {
        return None;
    }
    if let Some((disp, base)) = frame_index_address(dag, &am) {
        return Some([disp, base]);
    }
    if am.has_index_reg() || !am.has_base_reg() {
        return None;
    }
    let base = am.base_reg?;
    if let Some(disp) = symbolic_displacement(dag, &am) {
        return Some([disp, base]);
    }
    if am.disp == 0 {
        return None;
    }
    Some([i16_imm(dag, am.disp), base])
}

/// `SelectARII`: `[disp, base, index]`.
pub fn select_arii<D: AddrDag>(
    dag: &mut D,
    parent: Option<u32>,
    n: SDValue,
) -> Option<[SDValue; 3]> {
    let mut am = AddressMode::new(AddrType::Arii);
    if !match_address(dag, n, &mut am) || am.is_pc_relative(dag) {
        return None;
    }
    let (base_reg, index_reg) =
        (am.base_reg.filter(|_| am.has_base_reg())?, am.index_reg.filter(|_| am.has_index_reg())?);
    let (base, index) = if !is_address_base(dag, base_reg) && is_address_base(dag, index_reg) {
        (index_reg, base_reg)
    } else {
        (base_reg, index_reg)
    };
    if am.has_symbolic_displacement() {
        return None;
    }
    if am.disp == 0 && !allow_arii_with_zero_disp(dag, parent) {
        return None;
    }
    Some([i8_imm(dag, am.disp), base, index])
}

/// `SelectAL`: `[symbol or absolute]`.
pub fn select_al<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<SDValue> {
    let mut am = AddressMode::new(AddrType::Al);
    if !match_address(dag, n, &mut am) || am.is_pc_relative(dag) || am.has_base() || am.has_index_reg() {
        return None;
    }
    if let Some(symbol) = symbolic_displacement(dag, &am) {
        return Some(symbol);
    }
    (am.disp != 0).then(|| i32_imm(dag, am.disp))
}

/// `SelectPCD`: `[disp]`.
pub fn select_pcd<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<SDValue> {
    let mut am = AddressMode::new(AddrType::Pcd);
    if !match_address(dag, n, &mut am) || !am.is_pc_relative(dag) || am.has_index_reg() {
        return None;
    }
    Some(symbolic_displacement(dag, &am).unwrap_or_else(|| i16_imm(dag, am.disp)))
}

/// `SelectPCI`: `[disp, index]`.
pub fn select_pci<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<[SDValue; 2]> {
    let mut am = AddressMode::new(AddrType::Pci);
    if !match_address(dag, n, &mut am) || !am.is_pc_relative(dag) || !am.has_index_reg() {
        return None;
    }
    let index = am.index_reg?;
    Some([symbolic_displacement(dag, &am).unwrap_or_else(|| i8_imm(dag, am.disp)), index])
}

/// `SelectPCIBD`: `[disp, index, scale]`, scale 1.
pub fn select_pcibd<D: AddrDag>(
    dag: &mut D,
    n: SDValue,
) -> Option<[SDValue; 3]> {
    let [disp, index] = select_pci(dag, n)?;
    Some([disp, index, dag.target_constant(1, mvt::i8)])
}
