//! The matcher table's items: the pin's `BuiltinOpcodes` with their operands
//! decoded.
//!
//! # How it works
//! The pin's `llvm-tblgen -gen-dag-isel` writes `MatcherTable`, a byte string,
//! and `SelectCodeCommon` ([L3351][common]) reads it opcode by opcode. Here
//! each opcode is one [`Matcher`] and each operand is decoded, so the walk
//! reads no bytes. Jumps count items, not bytes: `Scope` is followed by one
//! [`Matcher::Alt`] per alternative (the items to the next one, 0 ends the
//! scope) and `SwitchOpcode`/`SwitchType` by one case header per case.
//!
//! Numbers are the pin's: [`Vt`] is `MVT::SimpleValueType`, opcodes are
//! `ISD::NodeType` and `X86ISD`, registers and target opcodes are the `X86::`
//! enums, all taken from the pin's headers by `tools/dagisel/regen.sh`.
//!
//! # Used by
//! `interp` walks it; `llrm-dagisel-gen` writes it.
//!
//! [common]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L3351

/// An `ISD::NodeType` or target node opcode, in the pin's numbering.
pub type Opcode = u16;

/// `MVT::SimpleValueType`, in the pin's numbering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Vt(pub u16);

/// Operand flags of `EmitNode`/`MorphNodeTo` (`OPFL_*`).
pub mod opfl {
    pub const CHAIN: u8 = 1;
    pub const GLUE_INPUT: u8 = 2;
    pub const GLUE_OUTPUT: u8 = 4;
    pub const MEM_REFS: u8 = 8;
    pub const VARIADIC_INFO: u8 = 15 << 4;

    /// `getNumFixedFromVariadicInfo`: how many root operands the variadic copy
    /// skips.
    pub const fn num_fixed_from_variadic_info(flags: u8) -> i32 {
        ((flags & VARIADIC_INFO) >> 4) as i32 - 1
    }
}

/// A run of `u16` entries in the table's operand list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slice {
    pub start: u32,
    pub len: u8,
}

/// The result types and operands of an `EmitNode` or `MorphNodeTo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emit {
    /// The machine opcode (`X86::ADD32rr`).
    pub opcode: u16,
    pub flags: u8,
    /// The first `num_vts` entries; chain and glue results follow from `flags`.
    pub vts: [Vt; 2],
    pub num_vts: u8,
    /// Recorded-node numbers, in the table's operand list.
    pub operands: Slice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Matcher {
    /// Starts alternatives: `Alt` items follow, each a candidate sequence.
    Scope,
    /// Items to the next alternative of the scope or case list this belongs to;
    /// 0 ends it.
    Alt(u32),
    RecordNode,
    RecordChild(u8),
    RecordMemRef,
    CaptureGlueInput,
    /// `OPC_CaptureDeactivationSymbol`: a trailing `ISD::DEACTIVATION_SYMBOL` operand (PAuth).
    CaptureDeactivationSymbol,
    MoveChild(u8),
    MoveSibling(u8),
    MoveParent,
    CheckSame(u8),
    CheckChildSame {
        child: u8,
        record: u8,
    },
    CheckPatternPredicate(u16),
    CheckPredicate(u16),
    CheckComplexPat {
        pattern: u8,
        record: u8,
    },
    CheckOpcode(Opcode),
    /// Followed by [`Matcher::OpcodeCase`] items, then `OpcodeCase { size: 0
    /// }`.
    SwitchOpcode,
    /// A case of `SwitchOpcode`: `size` items follow it.
    OpcodeCase {
        size: u32,
        opcode: Opcode,
    },
    CheckType(Vt),
    /// `OPC_CheckTypeRes`: the type of result `res` of the node.
    CheckTypeRes {
        res: u8,
        vt: Vt,
    },
    SwitchType,
    TypeCase {
        size: u32,
        vt: Vt,
    },
    CheckChildType {
        child: u8,
        vt: Vt,
    },
    CheckInteger(i64),
    CheckChildInteger {
        child: u8,
        value: i64,
    },
    CheckCondCode(u8),
    CheckChild2CondCode(u8),
    CheckValueType(Vt),
    CheckAndImm(u64),
    CheckOrImm(u64),
    CheckImmAllOnesV,
    CheckImmAllZerosV,
    CheckUndef,
    CheckFoldableChainNode,
    EmitInteger {
        vt: Vt,
        value: i64,
    },
    EmitRegister {
        vt: Vt,
        register: u16,
    },
    EmitConvertToTarget(u8),
    EmitMergeInputChains(Slice),
    EmitCopyToReg {
        record: u8,
        register: u16,
    },
    EmitNodeXForm {
        xform: u8,
        record: u8,
    },
    EmitNode(Emit),
    MorphNodeTo(Emit),
    CompleteMatch(Slice),
}

/// A table and the operand lists its `Emit`s and `CompleteMatch`es point into.
#[derive(Clone, Copy, Debug)]
pub struct Table {
    pub items: &'static [Matcher],
    pub operands: &'static [u16],
}
