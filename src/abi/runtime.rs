//! Port of `qbopt/abi/runtime.py`: what a call into BC's runtime can do to
//! the caller, routine by routine. The Python module docstring is the full
//! account.
//!
//! `TABLE` is the text of `runtime.toml`, included at build time, not its path.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::support::hash::IndexMap;

use iced_x86::{Code, Register};

use crate::abi::{callsite, events};
use crate::frontends::bc::blocks::{self, INLINE_TABLE};
use crate::objectfile::module::{self, Module};
use crate::objectfile::omf::ValueError;
use crate::support::pyrepr::{self, Repr};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Reg {
    Ax,
    Bx,
    Cx,
    Dx,
    Si,
    Di,
    Bp,
    Sp,
    Ds,
    Es,
    Flags,
}

impl Reg {
    /// Declaration order, which is `tuple(Reg)`.
    pub const ALL: [Reg; 11] = [
        Reg::Ax,
        Reg::Bx,
        Reg::Cx,
        Reg::Dx,
        Reg::Si,
        Reg::Di,
        Reg::Bp,
        Reg::Sp,
        Reg::Ds,
        Reg::Es,
        Reg::Flags,
    ];

    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            Reg::Ax => "AX",
            Reg::Bx => "BX",
            Reg::Cx => "CX",
            Reg::Dx => "DX",
            Reg::Si => "SI",
            Reg::Di => "DI",
            Reg::Bp => "BP",
            Reg::Sp => "SP",
            Reg::Ds => "DS",
            Reg::Es => "ES",
            Reg::Flags => "FLAGS",
        }
    }

    /// The `StrEnum` value.
    pub fn value(self) -> &'static str {
        match self {
            Reg::Ax => "ax",
            Reg::Bx => "bx",
            Reg::Cx => "cx",
            Reg::Dx => "dx",
            Reg::Si => "si",
            Reg::Di => "di",
            Reg::Bp => "bp",
            Reg::Sp => "sp",
            Reg::Ds => "ds",
            Reg::Es => "es",
            Reg::Flags => "flags",
        }
    }

    /// `Reg(value)`.
    pub fn from_value(value: &str) -> Result<Reg, String> {
        Reg::ALL
            .into_iter()
            .find(|one| one.value() == value)
            .ok_or_else(|| format!("{} is not a valid Reg", pyrepr::string(value)))
    }
}

/// A `StrEnum` orders as its string value.
impl Ord for Reg {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value().cmp(other.value())
    }
}

impl PartialOrd for Reg {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Repr for Reg {
    fn repr(&self) -> String {
        pyrepr::str_enum("Reg", self.name(), self.value())
    }
}

pub static EVERY: LazyLock<BTreeSet<Reg>> = LazyLock::new(|| Reg::ALL.into_iter().collect());

// cmacros' own PL/M convention: cBegin saves the routine's declared list and
// cEnd restores it, so a save list is a preservation guarantee whatever the
// body and its callees do (inc/cmacros.inc, mPush/mPop around the frame).
pub static PER_CONVENTION: LazyLock<BTreeSet<Reg>> =
    LazyLock::new(|| BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Flags]));

// Ordered by how much it concedes, so "can this write something the caller can
// name" is a comparison rather than a second table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Memory {
    None = 0,
    Arguments = 1, // only the values pushed for this call, popped before it returns
    Strings = 2,   // + any string descriptor or string data, anywhere
    // Its own data, and anything the caller handed it a pointer to. GCC's
    // ipa-modref splits a callee's effects the same way: writes at a fixed
    // address, and writes through parameter N. `ANY` says only "writes
    // memory", which is true of every routine here and tells a caller
    // nothing -- a global cannot be kept in a register across a PRINT, and
    // every program in the suite prints.
    //
    // Measured, not assumed. tools/runtime_writes.py reads a linked image:
    // over B_NBODY, B_ARRIDX and B_MATRIX, the runtime makes 287 writes to
    // a fixed address in DGROUP and not one of them names a cell in
    // BC_DATA, the segment BC puts a program's variables in. Nor does any
    // data-segment fixup in the corpus hand the runtime a pointer into it:
    // 151 are BC_CN -> BC_CN and one per program is BC_SA -> its code.
    Own = 3,
    Any = 4,
}

impl Memory {
    pub const ALL: [Memory; 5] = [
        Memory::None,
        Memory::Arguments,
        Memory::Strings,
        Memory::Own,
        Memory::Any,
    ];

    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            Memory::None => "NONE",
            Memory::Arguments => "ARGUMENTS",
            Memory::Strings => "STRINGS",
            Memory::Own => "OWN",
            Memory::Any => "ANY",
        }
    }

    /// The `IntEnum` value.
    pub fn value(self) -> i64 {
        self as i64
    }

    /// `Memory[name]`.
    pub fn from_name(name: &str) -> Result<Memory, String> {
        Memory::ALL
            .into_iter()
            .find(|one| one.name() == name)
            .ok_or_else(|| pyrepr::string(name))
    }
}

impl Repr for Memory {
    fn repr(&self) -> String {
        format!("<Memory.{}: {}>", self.name(), self.value())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Control {
    Returns,
    InlineTable, // comes back somewhere data after the call selects
    Never,
    Unknown,
}

impl Control {
    pub const ALL: [Control; 4] = [
        Control::Returns,
        Control::InlineTable,
        Control::Never,
        Control::Unknown,
    ];

    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            Control::Returns => "RETURNS",
            Control::InlineTable => "INLINE_TABLE",
            Control::Never => "NEVER",
            Control::Unknown => "UNKNOWN",
        }
    }

    /// The `StrEnum` value.
    pub fn value(self) -> &'static str {
        match self {
            Control::Returns => "returns",
            Control::InlineTable => "inline-table",
            Control::Never => "never",
            Control::Unknown => "unknown",
        }
    }

    /// `Control(value)`.
    pub fn from_value(value: &str) -> Result<Control, String> {
        Control::ALL
            .into_iter()
            .find(|one| one.value() == value)
            .ok_or_else(|| format!("{} is not a valid Control", pyrepr::string(value)))
    }
}

impl Repr for Control {
    fn repr(&self) -> String {
        pyrepr::str_enum("Control", self.name(), self.value())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Contract {
    pub name: String,
    pub cleanup: Option<i64>, // bytes the callee pops off the caller's stack
    pub control: Control,
    pub enters_user_code: bool,
    pub raises_error: bool,
    pub error_handling: bool,
    pub writes: Memory,
    pub reads: Memory,
    pub clobbers: BTreeSet<Reg>,
    pub established: bool,
    pub evidence: String,
    pub documented: Option<BTreeSet<Reg>>,
    // Registers the caller has to have set before the call. None is not
    // "none" -- it is "not established", and reads as every register, the
    // same way an unestablished contract clobbers every one. The asymmetry
    // is the point: over-stating a read only keeps a value alive, while
    // under-stating one deletes the instruction that produced it. B$FILD
    // takes its long in dx:ax, nothing recorded that, and removing the
    // moves that set it up printed FADD= 918528 for 1049600.
    pub inputs: Option<BTreeSet<Reg>>,
    // Values the call's ordinary, fall-through continuation observes. Most
    // routines use exactly ``inputs``. A control-transfer boundary may also
    // inspect register state on a hidden error/resume path, however, and
    // keeping that conservative all-path set in ``inputs`` must not turn
    // caller-saved scratch values into source-program operands on the direct
    // edge. None means the ordinary continuation has the same inputs.
    pub direct_inputs: Option<BTreeSet<Reg>>,
    // `clobbers` was proven over every path the call can return along,
    // whatever code those paths reach.
    pub clobbers_reached: bool,
    // Bytes the caller pops once the call returns: C's convention, where
    // `cleanup` is 0 and the arguments are still the caller's to release.
    pub caller_cleanup: i64,
    // 386 code: a register kept under the 8086 convention keeps only its
    // 16-bit half, and FS and GS are not kept at all.
    pub i386: bool,
    // A control-transfer routine's own footprint before it invokes or resumes
    // user code. `reads`/`writes` remain the transitive effect, which is ANY;
    // handler summarization stops at the transfer and consumes these instead.
    pub direct_writes: Option<Memory>,
    pub direct_reads: Option<Memory>,
}

impl Repr for Contract {
    /// Set elements print in `Reg` order, which is sorted repr order; Python
    /// prints them in hash order, which `tools/port_diff.py` sorts away.
    fn repr(&self) -> String {
        let regs = |set: &BTreeSet<Reg>| pyrepr::frozenset(&set.iter().collect::<Vec<_>>());
        pyrepr::dataclass(
            "Contract",
            &[
                ("name", self.name.repr()),
                ("cleanup", self.cleanup.repr()),
                ("control", self.control.repr()),
                ("enters_user_code", self.enters_user_code.repr()),
                ("raises_error", self.raises_error.repr()),
                ("error_handling", self.error_handling.repr()),
                ("writes", self.writes.repr()),
                ("reads", self.reads.repr()),
                ("clobbers", regs(&self.clobbers)),
                ("established", self.established.repr()),
                ("evidence", self.evidence.repr()),
                (
                    "documented",
                    self.documented.as_ref().map_or("None".to_owned(), regs),
                ),
                (
                    "inputs",
                    self.inputs.as_ref().map_or("None".to_owned(), regs),
                ),
                (
                    "direct_inputs",
                    self.direct_inputs.as_ref().map_or("None".to_owned(), regs),
                ),
                ("clobbers_reached", self.clobbers_reached.repr()),
                ("caller_cleanup", self.caller_cleanup.repr()),
                ("i386", self.i386.repr()),
                ("direct_writes", self.direct_writes.repr()),
                ("direct_reads", self.direct_reads.repr()),
            ],
        )
    }
}

pub fn worst(name: &str) -> Contract {
    Contract {
        name: name.to_owned(),
        cleanup: None,
        control: Control::Unknown,
        enters_user_code: true,
        raises_error: true,
        error_handling: false,
        writes: Memory::Any,
        reads: Memory::Any,
        clobbers: EVERY.clone(),
        established: false,
        evidence: "not established from the QuickBASIC 4.5 runtime source".to_owned(),
        documented: None,
        inputs: None,
        direct_inputs: None,
        clobbers_reached: false,
        caller_cleanup: 0,
        i386: false,
        direct_writes: None,
        direct_reads: None,
    }
}

pub fn preserves(routine: &Contract) -> BTreeSet<Reg> {
    EVERY.difference(&routine.clobbers).copied().collect()
}

/// Registers a call may return changed. The raise and the lowering must
/// both ask this: when the raise gave B$RND0 a fresh SI that the lowering
/// thought the call kept, the value BC relied on was never put in SI.
///
/// A routine reaching user code returns from code its clobber set does not
/// describe, unless that set was proven over everything it reaches.
pub fn disturbs(routine: &Contract) -> BTreeSet<Reg> {
    if (routine.enters_user_code || routine.error_handling) && !routine.clobbers_reached {
        return EVERY.clone();
    }
    routine.clobbers.clone()
}

// The order a routine's declared inputs are listed in. `Contract.inputs` is
// a set -- which register, not which position -- and the raise and the
// lowering both have to agree on a slot per input or they would pair an
// argument with somebody else's register. Any total order does; this is
// `Reg`'s own declaration order, so a register added there cannot be left
// out of a list written somewhere else.
pub const SLOTS: [Reg; 11] = Reg::ALL;

// What a routine's contract is under one toolchain, where the routines
// differ. Keyed by the compiler's own name for itself, which module.py
// reads off COMENT 0x00 -- the family is a fact about the object, and it
// reaches this map and nothing else.
//
// B$ENRA takes the frame size in cx. Disassembled from the linked image:
// PDS 7.1's is at 0x1d35 and QuickBASIC 4.5's at 0x211d, both reading cx
// through `push cx` and `sub sp,cx` before writing it, with no unresolved
// edge on any path from entry to the `jmp far` that returns. VBDOS's, at
// 0x397, also reads bx -- `or bx,bx` gates `call 02f7:0002` -- and that
// helper reaches `call far [di+24h]`, which no disassembly can follow, so
// VBDOS stays at the worst case.
//
// Python fills this across its module body, around `CONTRACTS`; this runs
// the same statements in the same order.
pub static VARIANTS: LazyLock<IndexMap<(&'static str, &'static str), Contract>> = LazyLock::new(
    || {
        let mut variants: IndexMap<(&'static str, &'static str), Contract> = IndexMap::default();

        for _family in ["pds71", "vbdos"] {
            variants.insert(
                ("B$RETA", _family),
                Contract {
                    inputs: Some(BTreeSet::from([
                        Reg::Ax,
                        Reg::Bx,
                        Reg::Cx,
                        Reg::Dx,
                        Reg::Si,
                        Reg::Di,
                    ])),
                    evidence: concat!(
                        "Shipped PDS/VBDOS gosub.asm RETA consumes the runtime frame and saved ",
                        "continuation, not caller arithmetic flags: DEC sets the tested SF; ",
                        "JCXZ tests a popped word. The event path enters EXSA, whose first CMP ",
                        "kills incoming flags (rtenexit.asm 004e/0068). The error path enters ",
                        "ERR_RG, which reaches XOR BH,BH before further dispatch (erproc.asm ",
                        "00ad/00c1). All six GP inputs retained conservatively; BP/SP, segments ",
                        "and direction remain runtime environment. No preservation, cleanup or ",
                        "ordinary-return claim. Frontend control flow leaves at RETA."
                    )
                    .to_owned(),
                    ..worst("B$RETA")
                },
            );
            for _name in ["B$ONTA", "B$ETT0", "B$ETT1", "B$ETT2"] {
                variants.insert(
                    (_name, _family),
                    Contract {
                        inputs: Some(BTreeSet::from([
                            Reg::Ax,
                            Reg::Bx,
                            Reg::Cx,
                            Reg::Dx,
                            Reg::Si,
                            Reg::Di,
                        ])),
                        evidence: concat!(
                            "BCL71ENR.LIB/VBDCL10E.LIB evttim.asm: ONTA at 002e/002f ",
                            "executes OR AX,DX at 003b/003c before every branch or call; ",
                            "ETT0/1/2 converge on XOR BL,BL at 0023/0024 before EVNT_SET. ",
                            "Incoming arithmetic flags cannot reach a dependency. Bound inputs ",
                            "by all six allocatable GP registers; segments, BP/SP and direction ",
                            "are runtime environment. No transitive preservation, cleanup, ",
                            "memory or control claim: error paths and indirect dependencies ",
                            "remain worst-case. See docs/optimizations/event-entry-blocker.md."
                        )
                        .to_owned(),
                        ..worst(_name)
                    },
                );
            }
        }

        for _family in ["qb45", "pds71", "vbdos"] {
            variants.insert(
            ("B$FCMD", _family),
            Contract {
                inputs: Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
                cleanup: Some(0),
                evidence: concat!(
                    "BCOM45.LIB/BCL71ENR.LIB/VBDCL10E.LIB oscmd.asm B$FCMD at 1:0023: ",
                    "balanced local saves and RETF (0041 in QB/PDS, 004a in VBDOS), no caller arguments. ",
                    "Its first call is B$CmdCopy at 1:0000, whose XOR BX,BX at 0002 kills incoming ",
                    "arithmetic flags before any conditional use or further call. Bound inputs by all ",
                    "six allocatable GP registers; segments, BP/SP and direction are runtime environment. ",
                    "Heap helper dependencies are incomplete, so memory, clobber and control effects ",
                    "remain worst-case; no preservation or termination claim."
                )
                .to_owned(),
                ..worst("B$FCMD")
            },
        );
        }

        // PDS 7.1 has the same zero-argument clock interface as the independently
        // audited QB 4.5 and VBDOS libraries.  Keep the profile-specific evidence
        // here: source lowering asks the shared table, rather than teaching the
        // frontend a runtime-family exception.
        variants.insert(
            ("B$TIMR", "pds71"),
            Contract {
                inputs: Some(BTreeSet::from([
                    Reg::Ax,
                    Reg::Bx,
                    Reg::Cx,
                    Reg::Dx,
                    Reg::Si,
                    Reg::Di,
                ])),
                cleanup: Some(0),
                evidence: concat!(
                    "BCL71ENR.LIB rt/ostimer.asm B$TIMR 0000..0042: balanced BP/SI saves; ",
                    "DOS GETTIM at 0004; MUL CH at 000a kills incoming arithmetic flags; ",
                    "the result is stored through BX at 0037..003b and its near pointer ",
                    "returned in AX by XCHG BX,AX; POP SI/BP / RETF consumes no caller ",
                    "arguments. Time, memory, x87, errors and GP effects remain conservative."
                )
                .to_owned(),
                ..worst("B$TIMR")
            },
        );

        for _one in ["pds71", "qb45"] {
            _entry(&mut variants, _one);
        }

        variants.insert(
        ("B$ENRA", "vbdos"),
        Contract {
            inputs: Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
            evidence: concat!(
                "VBDCL10E.LIB rtenexit.asm 0017..0062: saves the far return address, ",
                "XOR AX,AX at 001f overwrites incoming arithmetic flags, builds the frame ",
                "using CX, then OR BX,BX selects HFirstAllocBlock. Retain all GP inputs ",
                "without claiming allocator preservation, cleanup, termination or error ",
                "behavior. HFirstAllocBlock -> HandleAlloc reaches allocation/compaction ",
                "dependencies whose effects remain unknown. BP/SP, segments and direction ",
                "are runtime environment. Only a separately proven BX=0 site narrows this. ",
                "Library SHA256 59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301."
            )
            .to_owned(),
            ..worst("B$ENRA")
        },
    );

        for _name in ["B$SIN4", "B$SIN8", "B$COS4", "B$COS8"] {
            variants.insert(
            (_name, "vbdos"),
            Contract {
                inputs: Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
                cleanup: Some(0),
                evidence: concat!(
                    "VBDCL10E.LIB 87btrig.asm SIN4/SIN8 share 005a: ST0 operand, ",
                    "local BP frame, hardware exits 0093..0096 and 00c3..00c6 restore SP/BP and RETF; ",
                    "emulator tail B$EMSIN (embtrig.asm 0065) exits 00c8..00cb identically. ",
                    "COS4/COS8 share 0007 with the same local frame: hardware exits ",
                    "0040..0043 or 00c3..00c6; emulator tail B$EMCOS (embtrig.asm 0000) ",
                    "restores SP/BP and RETF at 0061..0064. ",
                    "All GP inputs are conservatively retained; no preservation, purity, x87 ",
                    "optimization or error-path guarantee is inferred. Error tails remain unknown."
                )
                .to_owned(),
                ..worst(_name)
            },
        );
        }

        variants.insert(
            ("B$POW4", "vbdos"),
            Contract {
                inputs: Some(BTreeSet::from([
                    Reg::Ax,
                    Reg::Bx,
                    Reg::Cx,
                    Reg::Dx,
                    Reg::Si,
                    Reg::Di,
                ])),
                cleanup: Some(0),
                evidence: concat!(
                    "VBDCL10E 87btran.asm 00e2: x87 operands, PUSH BP / MOV BP,SP. ",
                    "FXAM/FNSTSW then AND AL,47h at 00f3 overwrites arithmetic flags ",
                    "before dispatch. Normal paths join POP BP / RETF at 0089, 0098, ",
                    "00b3 or 00d8; logarithm/exponential kernel reached by relocated ",
                    "near jump 010b -> 0038 uses the same frame. Other paths tail ",
                    "B$RUNERR (overflow/domain); all GP, memory, x87 and error effects ",
                    "remain conservative. No algebraic replacement or purity claim."
                )
                .to_owned(),
                ..worst("B$POW4")
            },
        );

        let pow4 = variants[&("B$POW4", "vbdos")].clone();
        variants.insert(
            ("B$POW8", "vbdos"),
            Contract {
                name: "B$POW8".to_owned(),
                evidence: concat!(
                    "VBDCL10E 87btran.asm PUBDEFs POW4 and POW8 both resolve to ",
                    "segment 1 offset 00e2: identical entry and dependency graph. "
                )
                .to_owned()
                    + &variants[&("B$POW4", "vbdos")].evidence,
                ..pow4
            },
        );

        for (_name, _evidence) in [
            (
                "B$FLOF",
                concat!(
                    "loclof.asm 0049 enters 0035 and calls LocateFDB (dvcore.asm 00e2); ",
                    "XOR SI,SI at 00e6 overwrites arithmetic flags before lookup/dispatch."
                ),
            ),
            (
                "B$GET3",
                concat!(
                    "dvgetput.asm 0043 enters local 00a4 with stacked position words; ",
                    "its first call is LocateFDB, which overwrites arithmetic flags before lookup."
                ),
            ),
            (
                "B$GET4",
                concat!(
                    "dvgetput.asm 0064 reads the record position and OR CX,CX at 006c ",
                    "overwrites arithmetic flags before validation and local 00a4 dispatch."
                ),
            ),
            (
                "B$SACT",
                concat!(
                    "farstr stcore.asm entry 01b6 loads the descriptor at BP+0Ah and ",
                    "CMP word [DI],0 at 01bf overwrites arithmetic flags before string allocation/copy."
                ),
            ),
            (
                "B$DSG0",
                concat!(
                    "rtinit.asm 00de consists only of MOV DS:[relocated global],DS / RETF; ",
                    "it consumes no GP register or arithmetic flags and has no dependencies."
                ),
            ),
            (
                "B$PUT3",
                concat!(
                    "dvgetput.asm 005d sets AL=5 and joins GET3 at 0048, calling local ",
                    "00a4; LocateFDB (dvcore.asm 00e2) kills arithmetic flags with XOR SI,SI at 00e6."
                ),
            ),
            (
                "B$SMID",
                concat!(
                    "farstr mid.asm 0000 first calls strutil.asm 0013 with BX=[BP+0Ah]; ",
                    "OR AX,AX at 0016 kills arithmetic flags before any branch or further call."
                ),
            ),
            (
                "B$SPAC",
                concat!(
                    "farstr strfcn.asm 0182 passes AL=20h and CX=[BP+6] to local 0191; ",
                    "OR CX,CX there kills arithmetic flags before allocation or an error tail."
                ),
            ),
            (
                "B$RND0",
                concat!(
                    "random.asm 0000 calls local 0033; MUL CX at 003b sets CF/OF, ",
                    "ADD BX,AX at 0045 sets arithmetic flags before the later ADC. ",
                    "The straight-line body updates the seed, stores an x87 result, ",
                    "and returns its address through XCHG BX,AX at 0075."
                ),
            ),
            (
                "B$ATN4",
                concat!(
                    "87btriga.asm 0000 establishes its frame with SUB SP,0Ah at 0003, ",
                    "overwriting incoming arithmetic flags before any branch or error tail. ",
                    "The x87 operand and runtime call remain unchanged."
                ),
            ),
            (
                "B$UBND",
                concat!(
                    "dynamic.asm 0133 reads the dimension from [BP+6]; OR DH,DH at 013b ",
                    "overwrites incoming arithmetic flags before the first branch at 013d. ",
                    "Descriptor reads, the B$DeLink call and the B$ERR_BS tail remain intact."
                ),
            ),
            (
                "B$SSEK",
                concat!(
                    "dkrandio.asm 0145 calls local 0114 then B$LocateFDB in dvcore.asm ",
                    "00e2; PUSH saves precede XOR SI,SI at 00e6 and CMP at 00e8, ",
                    "overwriting incoming arithmetic flags before any branch. Seeking stays a call."
                ),
            ),
            (
                "B$STRI",
                concat!(
                    "farstr strfcn.asm 01a3 loads AL and CX from stack arguments then ",
                    "calls local 0191, whose OR CX,CX overwrites incoming arithmetic flags ",
                    "before branching or allocation. String construction stays a call."
                ),
            ),
            (
                "B$FERL",
                concat!(
                    "error.asm 013b loads the stored error line into AX, XORs DX,DX ",
                    "at 013e and RETFs at 0140; no incoming arithmetic flag is read."
                ),
            ),
            (
                "B$RNZP",
                concat!(
                    "random.asm 0079 loads the stack argument at BP+0Ah, XORs its ",
                    "words at 0081, stores the seed and RETFs at 0088; the straight-line ",
                    "body reads no incoming arithmetic flag. Randomization stays a call."
                ),
            ),
        ] {
            variants.insert(
                (_name, "vbdos"),
                Contract {
                    inputs: Some(BTreeSet::from([
                        Reg::Ax,
                        Reg::Bx,
                        Reg::Cx,
                        Reg::Dx,
                        Reg::Si,
                        Reg::Di,
                    ])),
                    evidence: "VBDCL10E.LIB: ".to_owned()
                        + _evidence
                        + concat!(
                            " All GP inputs retained; ",
                            "cleanup, memory, preservation and transitive control/error effects ",
                            "remain unknown. No runtime operation is replaced."
                        ),
                    ..worst(_name)
                },
            );
        }

        for _name in ["B$STR4", "B$STR8"] {
            variants.insert(
                (_name, "vbdos"),
                Contract {
                    inputs: Some(BTreeSet::from([
                        Reg::Ax,
                        Reg::Bx,
                        Reg::Cx,
                        Reg::Dx,
                        Reg::Si,
                        Reg::Di,
                    ])),
                    evidence: concat!(
                        "VBDCL10E stringfp.asm entries 0000/001e pass AL=4/8 and ",
                        "BX=BP+6 to string.asm 001e (STR_COMMON). FOUTBX at ifout.asm ",
                        "0000 overwrites arithmetic flags with CMP at 0003 before ",
                        "dispatching to floating formatting. Retain all GP inputs. ",
                        "Wrappers end POP BP / RETF 4 or 8, but the dependency graph ",
                        "through FOUTBX and StrAlcTmpCopy has unproved stack paths: ",
                        "cleanup remains unknown, as do all memory/control/error effects. ",
                        "No preservation or arithmetic replacement is claimed."
                    )
                    .to_owned(),
                    ..worst(_name)
                },
            );
        }

        for _name in ["B$INT4", "B$INT8"] {
            variants.insert(
                (_name, "vbdos"),
                Contract {
                    inputs: Some(BTreeSet::from([
                        Reg::Ax,
                        Reg::Bx,
                        Reg::Cx,
                        Reg::Dx,
                        Reg::Si,
                        Reg::Di,
                    ])),
                    cleanup: Some(0),
                    evidence: concat!(
                        "VBDCL10E 87bint.asm: INT4/INT8 share 0016, saving BP/SI/DI; ",
                        "BX=6 and AX=0400h call emulator.asm segment 2 entry 002a. ",
                        "CMP BX,0Ch overwrites incoming arithmetic flags. The relocated ",
                        "table base is 0010; index 6 selects word 001c -> 0637. ",
                        "Hardware path changes rounding control, FRNDINT, restores control ",
                        "and RET at 0660. Software path calls 1be3 and 06e2; exception ",
                        "dispatch includes indirect calls, INT 21h and IRET, so all ",
                        "control/error/memory/x87 effects remain unknown. Normal wrapper ",
                        "return restores SI/DI, MOV SP,BP / POP BP / RETF at 0028..002b: ",
                        "zero caller argument cleanup. No replacement or preservation claim."
                    )
                    .to_owned(),
                    ..worst(_name)
                },
            );
        }

        variants.insert(
        ("B$PEOS", "vbdos"),
        Contract {
            inputs: Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
            evidence: concat!(
                "VBDCL10E.LIB prnval.asm 018f: establishes BP, saves ES/SI, loads FInput ",
                "and OR AL,AL at 0197 kills incoming arithmetic flags before any branch/call. ",
                "All GP inputs retained; BP/SP, segments and direction are runtime environment. ",
                "Terminal input relocates the frame at 019d..01b7; disk/print paths skip it. ",
                "Cleanup remains unknown, as do all memory, clobber, control and error effects."
            )
            .to_owned(),
            ..worst("B$PEOS")
        },
    );

        variants.insert(
            ("B$EXTS", "vbdos"),
            Contract {
                inputs: Some(BTreeSet::from([
                    Reg::Ax,
                    Reg::Bx,
                    Reg::Cx,
                    Reg::Dx,
                    Reg::Si,
                    Reg::Di,
                ])),
                direct_inputs: Some(BTreeSet::new()),
                cleanup: Some(0),
                evidence: concat!(
                    "VBDCL10E.LIB rtenexit.asm 012e..0149: CMP BP,[runtime frame] ",
                    "kills incoming arithmetic flags. Both conditional branches and the ",
                    "state-clearing path reach RETF at 0149; no pushes, pops or incoming ",
                    "GP-register dependencies on the ordinary return edge. ",
                    "Writes globals and [bp-12h]; all GP inputs and unknown memory/clobber/",
                    "control/error effects retained. Library SHA256 ",
                    "59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301."
                )
                .to_owned(),
                ..worst("B$EXTS")
            },
        );

        variants.insert(
            ("B$FEVS", "vbdos"),
            Contract {
                inputs: Some(BTreeSet::from([
                    Reg::Ax,
                    Reg::Bx,
                    Reg::Cx,
                    Reg::Dx,
                    Reg::Si,
                    Reg::Di,
                ])),
                evidence: concat!(
                    "VBDCL10E osstmt.asm 01c9: establishes BP, saves SI/DI/DS, then calls ",
                    "B$RefStringArgLast (farstr/strutil.asm 0010). That helper loads ",
                    "[bp+6], then OR AX,AX at 0016 overwrites incoming arithmetic flags ",
                    "before any flag use or further call. All GP inputs and unknown ",
                    "memory/clobber/control/error effects retained. Cleanup remains unknown ",
                    "because the transitive dependency audit is incomplete. Library SHA256 ",
                    "59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301."
                )
                .to_owned(),
                ..worst("B$FEVS")
            },
        );

        variants.insert(
            ("B$CHOU", "vbdos"),
            Contract {
                inputs: Some(BTreeSet::from([
                    Reg::Ax,
                    Reg::Bx,
                    Reg::Cx,
                    Reg::Dx,
                    Reg::Si,
                    Reg::Di,
                ])),
                cleanup: Some(2),
                evidence: concat!(
                    "VBDCL10E.LIB pr0a.asm 0055: PUSH BP / MOV BP,SP; reads channel at [bp+6], ",
                    "calls B$ChkFNUM and B$SetFNum, POP BP / RETF 2 at 0061..0064. ",
                    "All GP inputs and unknown clobber/memory/control/error effects retained; ",
                    "only the normal-return argument cleanup is established. Library SHA256 ",
                    "59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301."
                )
                .to_owned(),
                ..worst("B$CHOU")
            },
        );

        for (_name, _cleanup, _evidence) in [
            (
                "B$CSCN",
                None,
                concat!(
                    "VBDCL10E gwscreen.asm 0000 calls ScSetup (locate.asm 001c): ",
                    "POP BX saves the near return, then BP/ES/SI are saved; SHL AX,1 ",
                    "at 0027 kills incoming arithmetic flags. ScCleanUpParms 000f ",
                    "restores these saves, pops the far return and count, and advances ",
                    "SP by twice the count before RETF. Cleanup is variable, not zero. ",
                    "SCRSTT calls indirect screen handlers; display, alias and error ",
                    "effects remain unknown."
                ),
            ),
            (
                "B$WIDT",
                Some(4),
                concat!(
                    "VBDCL10E ioscrn.asm 002b establishes BP and calls EnsureFI, whose ",
                    "CMP at gwini.asm 0000 kills incoming arithmetic flags. Arguments ",
                    "at BP+8/+6 feed SWIDTH; normal exit POP BP / RETF 4 at 006d. ",
                    "Invalid width tails ERR_FC; BIOS/display and error effects unknown."
                ),
            ),
            (
                "B$SLEP",
                Some(4),
                concat!(
                    "VBDCL10E evtkey.asm 00b3 establishes BP; local 0157 CMP kills ",
                    "incoming flags. SetKybdInt saves/restores DS/DX/AX/BX/ES across ",
                    "BIOS/DOS interrupts and RETF at llcevt.asm 005b. SetClockInt ",
                    "restores its saves and RET at llaevt.asm 00c4. SleepInit 0101 ",
                    "calls stack-balanced tick conversion 00c9..0100. Event wait ",
                    "joins POP BP / RETF 4 at 00e6; interrupt/control effects unknown."
                ),
            ),
            (
                "B$TIMR",
                Some(0),
                concat!(
                    "VBDCL10E ostimer.asm 0000 saves BP/SI, obtains DOS time via INT 21h ",
                    "and MUL CH at 000a kills incoming arithmetic flags. Both relocated ",
                    "integer-to-float calls target member 299 entry 0000: PUSH BX / ",
                    "FILD word [BX] / POP BX / RET. Local arithmetic helper 0043..0063 ",
                    "balances DX/AX saves. POP SI/BP / RETF at 0040 returns a pointer ",
                    "to the stored float in AX; time, memory, x87 and errors remain unknown."
                ),
            ),
            (
                "B$FRI2",
                Some(2),
                concat!(
                    "VBDCL10E stfree.asm 004d saves BP/SI; XOR CX,CX at 0053 kills ",
                    "incoming flags. FRE selectors join POP SI/BP / RETF 2 at 00b0. ",
                    "FHCompact/FHByteSize return near; CbCompactHeap (lmem segment 5 ",
                    "0066..00aa) and GAFC (getactiv 0000..0097) consume their two ",
                    "internal argument words with RETF 4. Alternate entries 009d/009e ",
                    "are INC BX, not extra pushes. Heap compaction, aliases and errors ",
                    "remain unknown; no register preservation inferred."
                ),
            ),
            (
                "B$STI4",
                Some(4),
                concat!(
                    "VBDCL10E farstr/string.asm 000f reads the long at BP+6 through ",
                    "STR_COMMON 001e. FOUTBX (ifout.asm 0000) kills incoming flags ",
                    "with CMP at 0003; integer path returns near at 0050. Common ",
                    "copy via StrAlcTmpCopy (strutil.asm 01f5..0209) balances its ",
                    "local saves; public POP BP / RETF 4 at 001a. Formatting, heap ",
                    "allocation, aliases and errors remain unknown."
                ),
            ),
            (
                "B$FMKI",
                Some(2),
                concat!(
                    "rt/strnum.asm 005e..0070: SI addresses the word argument at BP+6, ",
                    "CX=2; StrAlcTmpCopy at strutil.asm 01f5 calls AlcTmpSH, whose CMP ",
                    "at 0103 overwrites incoming arithmetic flags before dependencies. ",
                    "Copy returns near; POP SI/BP / RETF 2. Allocation, aliases and errors ",
                    "remain unknown; no register preservation inferred from local saves."
                ),
            ),
            (
                "B$FMKL",
                Some(4),
                concat!(
                    "rt/strnum.asm 0073..0085: SI addresses the dword argument at BP+6, ",
                    "CX=4; StrAlcTmpCopy -> AlcTmpSH overwrites incoming arithmetic flags ",
                    "at strutil.asm 0103. Near copy return followed by POP SI/BP / RETF 4. ",
                    "Heap writes, aliases, errors and register clobbers remain unknown."
                ),
            ),
            (
                "B$FCVI",
                Some(2),
                concat!(
                    "rt/strnum.asm 0024..0041: XOR CH,CH at 0029 replaces incoming ",
                    "arithmetic flags. PUSH CS / near CALL 0000 balances the local RETF. ",
                    "Helper reads the string through RefString, checks length, copies two ",
                    "bytes and calls DelTempSH, or tails ERR_FC. Normal POP DI/BP / RETF 2; ",
                    "temporary deletion, memory writes and errors remain unknown."
                ),
            ),
            (
                "B$FCVS",
                Some(2),
                concat!(
                    "rt/strnum.asm 0044..005b: XOR CH,CH at 0049 replaces incoming ",
                    "arithmetic flags. PUSH CS / near CALL 0000 balances the local RETF. ",
                    "Shared conversion helper checks length and copies four bytes, with ",
                    "RefString and DelTempSH dependencies or ERR_FC. POP DI/BP / RETF 2; ",
                    "no purity, preservation or error-path guarantee."
                ),
            ),
            (
                "B$LEFT",
                Some(4),
                concat!(
                    "farstr/strfcn.asm 00dd..00f3: descriptor/count at BP+8/+6; first ",
                    "dependency RefString (strutil.asm 0013) overwrites arithmetic flags ",
                    "at 0016 before branching. Two internal words passed to substring ",
                    "wrapper 0113, whose RET 4 balances them; POP BP / RETF 4 at 00f2. ",
                    "Allocation, temporary deletion and errors remain unknown."
                ),
            ),
            (
                "B$RGHT",
                Some(4),
                concat!(
                    "farstr/strfcn.asm 00c9: RefString first overwrites incoming arithmetic ",
                    "flags. Reads count at BP+6, computes start from length, then joins ",
                    "LEFT at 00e9 or 00eb. Substring wrapper 0113 consumes two internal ",
                    "words with RET 4; shared normal exit is POP BP / RETF 4 at 00f2. ",
                    "Allocation, temporary deletion and errors remain unknown."
                ),
            ),
            (
                "B$RTRM",
                Some(2),
                concat!(
                    "farstr/strfcn.asm 0215..0253: RefStringArgLast overwrites arithmetic ",
                    "flags before branching; reverse space scan uses STD then CLD. Substring ",
                    "helper 011d returns near; normal exit POP DI/BP / RETF 2. Allocation, ",
                    "temporary deletion, aliases and errors remain unknown."
                ),
            ),
            (
                "B$LCAS",
                Some(2),
                concat!(
                    "VBDCL10E farstr/strfcn.asm 01e8..0213: descriptor at BP+6 via ",
                    "RefStringArgLast; RefString OR AX,AX at strutil.asm 0016 replaces ",
                    "incoming arithmetic flags. Empty path pops saved DX and joins 0208; ",
                    "nonempty path copies/allocates through helper 0000 and converts bytes. ",
                    "The indirect CALL BX is fixed by the 01e9 relocation to B$ToLower ",
                    "(gwini.asm 00a7..00e0, RETF), balanced by PUSH CS / near CALL. ",
                    "Both normal paths restore DS/DI/SI/BP then RETF 2 at 020c. ",
                    "Allocation, aliases, register preservation and error effects remain unknown."
                ),
            ),
            (
                "B$FASC",
                Some(2),
                concat!(
                    "farstr/strfcn.asm 004e..0062: RefStringArgLast first, then a byte read ",
                    "or ERR_FC for empty strings. DelTempSH -> DelString -> FreeDataPpv can ",
                    "delete a temporary. Normal return POP BP / RETF 2; RefString's OR AX,AX ",
                    "replaces incoming flags. No purity or heap preservation claim."
                ),
            ),
            (
                "B$FCHR",
                Some(2),
                concat!(
                    "farstr/stcore.asm 0264..027c: AlcTmpSH first (strutil.asm 00fe; CMP BX ",
                    "at 0103 overwrites flags before dependencies), then writes the byte or ",
                    "tails ERR_FC for a nonzero high byte. Normal POP DI/BP / RETF 2. ",
                    "Allocation/error dependencies stay unknown; all GP inputs retained."
                ),
            ),
            (
                "B$LNIN",
                Some(10),
                concat!(
                    "rt/lininp.asm 0000..006e: initial CMP at 0004 overwrites arithmetic ",
                    "flags before terminal/disk dispatch. Both normal paths join string ",
                    "assignment (six internal words, ASSN RETF 12), InpReset, POP SI/BP ",
                    "and RETF 10. InpReset is a near reset, not PEOS's frame-relocating ",
                    "epilogue. FillBuf has indirect device calls; input, alias, control and ",
                    "error effects remain unknown. Cleanup describes only normal return."
                ),
            ),
            (
                "B$ERS1",
                Some(2),
                concat!(
                    "rt/recarray.asm 014b..016e reads descriptor [bp+6], XOR CX,CX at ",
                    "0152 overwrites incoming arithmetic flags. Empty and nonempty paths ",
                    "join POP SI/BP / RETF 2. Nonempty calls FreePpv with two words ",
                    "(RETF 4 at lmove.asm 01da), then stack-neutral DeLink at 0075..008f. ",
                    "FreePpv -> FreeHandle mutates heap state; no alias/preservation claim."
                ),
            ),
            (
                "B$ASSN",
                Some(12),
                concat!(
                    "farstr/string.asm 005e..00b3 reads six stack words. OR AX,AX at 0075 ",
                    "replaces incoming arithmetic flags before branches/dependencies. Fixed ",
                    "copy/padding, LDFS -> SAS1, and LSET paths join POP DI/SI/BP / RETF 12. ",
                    "LDFS consumes 6 internal bytes, SAS1 4, LSET 8. Allocation, alias writes ",
                    "and error paths remain unknown; string direction is runtime environment."
                ),
            ),
            (
                "B$SCMP",
                Some(4),
                concat!(
                    "farstr/stcore.asm 0280..02b8 reads two descriptors; first RefString ",
                    "dependency replaces incoming arithmetic flags (strutil.asm 0016 OR AX,AX). ",
                    "RefStringArgLast is the same reader with [bp+6]. DelStrTemp calls ",
                    "DelString -> FreeDataPpv for temporaries; near returns are stack neutral. ",
                    "Comparison saves/reinstates its flags around deletion, then restores ",
                    "DS/DI/SI/BP and RETF 4. Heap effects and all clobbers remain conservative."
                ),
            ),
            (
                "B$SCPF",
                Some(2),
                concat!(
                    "farstr/string.asm 01aa..01c0 calls SCPY (0164, RETF 2) then STDL ",
                    "(0171, RETF 2), each with one internal argument; POP AX/BP / RETF 2 ",
                    "returns to caller. SCPY -> StrAlcTmpCopySH -> RefString overwrites ",
                    "incoming flags before conditional work; STDL -> DelString -> FreeDataPpv. ",
                    "Allocation/freeing and errors remain unknown, not a pure copy."
                ),
            ),
            (
                "B$FMID",
                Some(6),
                concat!(
                    "farstr/strfcn.asm 00f6..0112 reads descriptor/start/count at [bp+0a/08/06]. ",
                    "First dependency RefString (strutil.asm 0013) overwrites incoming flags ",
                    "with OR AX,AX at 0016 before branching; it has no dependencies or stack ",
                    "adjustments and returns near. The substring wrapper 0113..011c consumes ",
                    "two internal words with RET 4; public normal return is POP BP / RETF 6. ",
                    "Invalid ranges tail ERR_FC and allocation/freeing remain unknown effects; ",
                    "no purity, preservation or error-path guarantee."
                ),
            ),
            (
                "B$FLEN",
                Some(2),
                concat!(
                    "farstr/stcore.asm 02b9 reads a far-string descriptor at [bp+6]; OR AX,AX ",
                    "at 02c1 kills incoming arithmetic flags before any branch/dependency. ",
                    "Empty and nonempty paths join POP BP / RETF 2 at 02f6..02f9. ",
                    "Temporary strings call FreeDataPpv with two words (RETF 4 at 01da), ",
                    "which calls FreeHandle (RET at 010c). Freeing may mutate aliased heap ",
                    "state; no memory or register preservation is claimed."
                ),
            ),
            (
                "B$RDIM",
                None,
                concat!(
                    "erase.asm 0000 establishes BP and reads descriptor [bp+6]; OR BL,BL ",
                    "at 0009 kills incoming flags before branches or dependencies. All GP ",
                    "inputs retained. DIM_COMMON consumes rank-dependent stack arguments, ",
                    "so cleanup stays unknown unless separately proven at the call site."
                ),
            ),
            (
                "B$FEOF",
                Some(2),
                concat!(
                    "dvstmt.asm 0073 reads file word [bp+6]; OR BX,BX kills incoming flags. ",
                    "File and DOS console paths join POP BP / RETF 2 at 0097..009a; ",
                    "invalid console mode tails ERR_IFN. DOS/device/error effects remain unknown."
                ),
            ),
            (
                "B$CLOS",
                None,
                concat!(
                    "dvcore.asm 01b4 reads a stack count and file words; JCXZ selects CLOSF ",
                    "or LocateFDB. CLOSF's CMP at 019f and LocateFDB's XOR SI,SI at 00e6 ",
                    "kill incoming flags before dependencies. Return restores SP from the advanced ",
                    "argument cursor at 01e3, so cleanup stays unknown, not zero."
                ),
            ),
            (
                "B$ERAS",
                Some(2),
                concat!(
                    "erase.asm 0020 reads descriptor [bp+6]; empty arrays go directly to epilogue; ",
                    "other paths test descriptor flags before dependencies. All normal paths join ",
                    "POP DI/SI/BP / RETF 2 at 00bf..00c4. Heap, alias and error effects remain unknown."
                ),
            ),
            (
                "B$OPEN",
                Some(8),
                concat!(
                    "dkutil.asm 00c0..00f4 reads four stack words, calls DOS3CHECK and OPENIT, ",
                    "and restores BP then RETF 8 at 00f1. Other branches tail ERR_AFE or ERR_IFN; ",
                    "device, allocation and error effects are not established."
                ),
            ),
            (
                "B$DSKI",
                Some(2),
                concat!(
                    "inpdsk.asm 0016..0060 reads [bp+6], calls ChkFNUM/LocateFDB/EnsureFI, ",
                    "sets input state, restores SI/BP and RETF 2 at 005e. Other branches tail ",
                    "ERR_IFN, ERR_RPE or ERR_BFM; no device or error-path guarantees."
                ),
            ),
            (
                "B$FDR1",
                None,
                concat!(
                    "VBDCL10E rt/dkdir.asm 0003 sets search mode, establishes BP, and ",
                    "sets DOS DTA (AH=1Ah, DS:DX); TEST at 0014 replaces arithmetic flags ",
                    "before search dispatch. RefStringArgLast reads BP+6; GET_PATHNAME, ",
                    "DelTempSH, DOS find-first/find-next, GetZStrLen and StrAlcTmpCopy ",
                    "perform path, directory and heap work. Shared exit restores DI/SI/BP ",
                    "then mode-selects RETF or RETF 2 (007e/007f); cleanup remains unknown ",
                    "rather than assuming the shared mode survives every dependency. ",
                    "All GP inputs retained; memory, aliases, control and errors unknown."
                ),
            ),
            (
                "B$FREF",
                Some(0),
                concat!(
                    "dvstmt.asm 0031..0051 walks B$NextFDB, restores SI/BP and RETF. ",
                    "NextFDB saves AX/BX/CX/DX and calls PpvWalkHeap with two words; ",
                    "lwalk.asm PpvWalkHeap returns RETF 4 at 0039. No caller arguments."
                ),
            ),
            (
                "B$LDFS",
                Some(6),
                concat!(
                    "string.asm 0030..005b loads [bp+6/+8/+0a], optionally calls B$AlcTmpSH ",
                    "and copies bytes; both zero-length and copying paths restore DS/DI/SI/BP ",
                    "then RETF 6. Allocator and error dependencies remain unproved."
                ),
            ),
        ] {
            variants.insert(
            (_name, "vbdos"),
            Contract {
                inputs: Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
                cleanup: _cleanup,
                evidence: "VBDCL10E.LIB: ".to_owned()
                    + _evidence
                    + concat!(
                        " All GP inputs and unknown effects retained; cleanup only where stated. ",
                        "SHA256 59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301."
                    ),
                ..worst(_name)
            },
        );
        }

        // VBDOS's array eraser receives its descriptor entirely on the stack. The
        // broad ``inputs`` set above remains the conservative bound for unresolved
        // dependency/error transfers, while the ordinary return edge observes no
        // incoming GP value. Keeping both facts separate prevents unrelated helper
        // clobbers in the caller from becoming semantic operands solely because ERASE
        // follows them.
        let eras = variants[&("B$ERAS", "vbdos")].clone();
        variants.insert(
            ("B$ERAS", "vbdos"),
            Contract {
                direct_inputs: Some(BTreeSet::new()),
                ..eras
            },
        );

        // Fixed-length UDT/string assignment likewise receives source/destination far
        // pointers and both byte counts in its six stack words.  The implementation
        // evidence above retains unknown hidden/error transfers, but its ordinary
        // returning arm has no additional caller-register argument.
        let assn = variants[&("B$ASSN", "vbdos")].clone();
        variants.insert(
            ("B$ASSN", "vbdos"),
            Contract {
                direct_inputs: Some(BTreeSet::new()),
                ..assn
            },
        );

        // Emission-facing interfaces for QB45 routines newly reached by the demo
        // corpus. These deliberately do not turn into complete contracts: each call
        // keeps worst-case memory, clobber, control and error effects. The sole claim
        // needed here is that allocation may reproduce BC's incoming GP state; stack
        // cleanup is recorded where the runtime's own epilogue makes it fixed.
        for (_name, _cleanup, _evidence) in [
            (
                "B$TIMR",
                Some(0),
                concat!(
                    "rt/ostimer.asm B$TIMR declares no parameters. CALLOS GETTIM supplies ",
                    "CH/CL/DH/DL, and MUL CH overwrites arithmetic flags before any ",
                    "conditional use. BCOM45.LIB member 170, offset 0000 ends POP ES/SI/BP ",
                    "/ RETF at 0041..0044. It consumes no caller argument."
                ),
            ),
            (
                "B$CSCN",
                None,
                concat!(
                    "rt/gwscreen.asm takes a count-led parameter block entirely on the ",
                    "stack. B$ScSetup in rt/gwscr.asm pops its near continuation, establishes ",
                    "BP, reads the count at BP+6, and SHL AX,1 overwrites incoming arithmetic ",
                    "flags before dispatch. B$ScCleanUpParms removes twice the runtime count; ",
                    "cleanup is variable rather than zero."
                ),
            ),
            (
                "B$BLOD",
                Some(6),
                concat!(
                    "rt/bload.asm declares three ParmW stack arguments. Entry establishes BP, ",
                    "sets AX, loads DX from BP+0Ah and initializes runtime globals before its ",
                    "first dependency; subsequent branches consume values or flags produced ",
                    "inside the routine. BCOM45.LIB member 31, offset 0058 ends POP BP / RETF ",
                    "6 at 009e..009f."
                ),
            ),
            (
                "B$SCLS",
                Some(2),
                concat!(
                    "rt/gwscr.asm `cProc B$SCLS,<PUBLIC,FAR>` with one `parmW ScnNum`; ",
                    "`MOV BX,ScnNum` / `INC BX` sets flags before any branch. cEnd is RETF 2. ",
                    "Out-of-range parameters tail B$ERR_FC."
                ),
            ),
            (
                "B$INKY",
                Some(0),
                concat!(
                    "rt/stinkey.asm `cProc B$INKY,<FAR,PUBLIC,FORCEFRAME>`, `sd * pascal ",
                    "B$INKY(void)`: no parameters, descriptor returned in AX. TEST b$IOFLAG ",
                    "sets flags before any branch. cEnd is RETF; redirected end of input ",
                    "jumps to B$END instead."
                ),
            ),
            (
                "B$SCMP",
                Some(4),
                concat!(
                    "rt/stcore.asm `XOR DX,DX` falls into `cProc SCMP,<FAR>,<ES,SI,DI>` with ",
                    "`parmW psdL` and `parmW psdR`, so RETF 4. The result is the flags of ",
                    "the comparison, saved across B$STDALCTMP by PUSHF/POPF."
                ),
            ),
            (
                "B$BSAV",
                Some(6),
                concat!(
                    "rt/bload.asm `cProc B$BSAV,<PUBLIC,FAR>` with three ParmW (pFileName, ",
                    "Offs, Len), so cEnd is RETF 6. The first instruction writes b$Buf3 ",
                    "and reads no register; it reads [b$seg]. Disk full tails B$ERR_DFL."
                ),
            ),
            (
                "B$POW4",
                Some(0),
                concat!(
                    "BCOM45.LIB 87btran.asm 0000: `mov dx,<selector>` / `mov byte [..],0` / ",
                    "`jmp` into 87bdisp.asm __ctrand2, whose exit at 0028 is a bare RETF: ",
                    "the operands are on the x87 stack and nothing is on the 8086 stack. ",
                    "The transcendental dispatch __trandisp2 is `jmp word [bx]`, and ",
                    "__FF_intrin_err reaches B$RUNERR."
                ),
            ),
        ] {
            variants.insert(
                (_name, "qb45"),
                Contract {
                    inputs: Some(BTreeSet::from([
                        Reg::Ax,
                        Reg::Bx,
                        Reg::Cx,
                        Reg::Dx,
                        Reg::Si,
                        Reg::Di,
                    ])),
                    cleanup: _cleanup,
                    evidence: concat!(
                        "QuickBASIC 4.5 runtime source and BCOM45.LIB SHA256 ",
                        "5b1c7a6fbb102e3e47acaa38349efa9bf1dae674e57f4d86d170e920086c8996: "
                    )
                    .to_owned()
                        + _evidence
                        + concat!(
                            " All six allocatable GP inputs are retained conservatively; BP/SP, ",
                            "segments and direction are runtime environment. Unknown memory, ",
                            "clobber, control and error effects remain unchanged."
                        ),
                    ..worst(_name)
                },
            );
        }

        // Read in their source above, and none reaches the program's code: B$SCLS's
        // one indirect call is the runtime's own viewport vector, B$INKY's other exit
        // is B$END, and B$POW4's is B$RUNERR, which `raises_error` already says.
        for _name in ["B$SCLS", "B$INKY", "B$SCMP", "B$BSAV", "B$POW4"] {
            let one = variants[&(_name, "qb45")].clone();
            variants.insert(
                (_name, "qb45"),
                Contract {
                    enters_user_code: false,
                    ..one
                },
            );
        }

        for (_name, _cleanup) in [
            ("B$PCR4", 4),
            ("B$PSR4", 4),
            ("B$PCR8", 8),
            ("B$PSR8", 8),
            ("B$PER8", 8),
        ] {
            variants.insert(
            (_name, "vbdos"),
            Contract {
                inputs: Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
                cleanup: Some(_cleanup),
                evidence: concat!(
                    "rt/prnvalfp.asm: scalar argument pushed by value; entry sets ",
                    "AL=VT_R4 (4) or VT_R8 (8), AH=terminator, then tails B$PRINT. ",
                    "VBDCL10E.LIB prnvalfp.asm entries 0000/0006/0012/0018/001e ",
                    "match; prnval.asm 003d saves the type and 0087 reloads it. ",
                    "Normal exits select RETF 4 at 009a or RETF 8 at 0094. ",
                    "Source PRINTX documents the same argument-width cleanup. ",
                    "Device dispatch and error effects remain unknown; all GP inputs retained. ",
                    "Library SHA256 59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301."
                )
                .to_owned(),
                ..worst(_name)
            },
        );
        }

        variants.insert(
        ("B$EXSA", "vbdos"),
        Contract {
            inputs: Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
            direct_inputs: Some(BTreeSet::from([Reg::Ax, Reg::Dx])),
            cleanup: Some(0),
            evidence: concat!(
                "VBDCL10E.LIB rtenexit.asm, seg 1:0x68: CMP [bp-12h],0 kills incoming arithmetic flags. ",
                "Bound inputs by all six allocatable GP registers, not by incomplete helper summaries. ",
                "The normal exit saves/restores DX:AX, restores the frame at 0x8e..0x97, then jumps ",
                "through the return address popped at 0x6e/0x72; it removes no caller arguments. ",
                "BP/SP and segments are the fixed runtime environment. Indirect/error/helper effects ",
                "remain unknown: no clobber, memory or control guarantee is relaxed."
            )
            .to_owned(),
            ..worst("B$EXSA")
        },
    );

        // B$EXSA under PDS 7.1, bounded from the linked image at 0x1d6c. Its
        // returning path -- `pop word [422h]`/`[424h]`, `lea sp,[bp-6]`, four pops,
        // `jmp far [422h]` -- reads no register and removes no caller argument, so
        // cleanup is 0. Its other path is error dispatch, and there the survivors
        // are what is declared here rather than what is read: bx is fully written
        // (`mov bl,13h` at 0x19da, `xor bh,bh` at 0x1a63) and ax by the entry
        // table, but cx survives the arm that skips `mov cx,[41Ch]`, and dx, si and
        // di are never written on any path before `jmp cx` at 0x1a79 or the `retf`
        // at 0x1b10. A superset, which costs a copy where it is wrong and cannot
        // be unsound; the control semantics past that indirect jump stay unknown,
        // which is what the conservative fields still say.
        variants.insert(
        ("B$EXSA", "pds71"),
        Contract {
            inputs: Some(BTreeSet::from([Reg::Ax, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
            direct_inputs: Some(BTreeSet::from([Reg::Ax, Reg::Dx])),
            cleanup: Some(0),
            evidence: concat!(
                "disassembled from the linked image at 0x1d6c: the returning path reads no register ",
                "and removes no argument; on the error path ax and bx are written before every ",
                "terminal, and cx, dx, si and di are not. The normal return continuation ",
                "exports dx:ax to the BASIC caller, so both halves are live at this boundary."
            )
            .to_owned(),
            ..worst("B$EXSA")
        },
    );

        // QuickBASIC 4.5's, at 0x20f2: no register read on any reachable path, one
        // exit, no unresolved edge.
        variants.insert(
            ("B$EXSA", "qb45"),
            Contract {
                inputs: Some(BTreeSet::from([Reg::Ax, Reg::Dx])),
                direct_inputs: Some(BTreeSet::from([Reg::Ax, Reg::Dx])),
                cleanup: Some(0),
                enters_user_code: false,
                evidence: concat!(
                    "disassembled from the linked image at 0x20f2: no register read, one exit; ",
                    "the normal return continuation exports dx:ax to the BASIC caller"
                )
                .to_owned(),
                ..worst("B$EXSA")
            },
        );

        for _family in ["qb45", "pds71", "vbdos"] {
            for _name in ["B$HARY", "B$LINA"] {
                variants.insert((_name, _family), CONTRACTS[_name].clone());
            }
        }

        for (_family, _library, _offset) in [
            ("pds71", "BCL71ENR.LIB", "0103"),
            ("vbdos", "VBDCL10E.LIB", "0127"),
        ] {
            variants.insert(
                ("B$EVK1", _family),
                Contract {
                    name: "B$EVK1".to_owned(),
                    evidence: format!(
                        "{_library} evtcore.asm PUBDEF: B$EVK1 and B$EVCK both name \
                     segment 1 offset {_offset}; exact entry aliases in this runtime family. \
                     Library hashes and scope: docs/optimizations/event-entry-blocker.md. "
                    ) + &CONTRACTS["B$EVCK"].evidence,
                    ..CONTRACTS["B$EVCK"].clone()
                },
            );
        }

        variants
    },
);

/// A call into the program's own code, by name.
///
/// BC compiles a SUB as a PUBDEF of the same object and calls it through
/// an EXTDEF fixup, so `Module.calls` names it like a runtime routine and
/// only the PUBDEF set tells the two apart. A BASIC argument arrives as a
/// far pointer on the stack, so nothing arrives in a register -- and that
/// is the only thing established here. Everything else stays at the worst
/// case: what such a procedure clobbers is as unknown as any other.
pub fn own(name: &str) -> Contract {
    Contract {
        inputs: Some(BTreeSet::new()),
        evidence: "a PUBDEF of this same module; BC pushes a far pointer per BASIC argument"
            .to_owned(),
        ..worst(name)
    }
}

/// Python mutates the module-level `VARIANTS`; this is handed the map
/// being built.
fn _entry(variants: &mut IndexMap<(&'static str, &'static str), Contract>, family: &'static str) {
    variants.insert(
        ("B$ENRA", family),
        Contract {
            inputs: Some(BTreeSet::from([Reg::Cx])),
            cleanup: Some(0),
            enters_user_code: false,
            evidence: concat!(
                "disassembled from the linked image: `push cx` then `sub sp,cx` before any write, ",
                "no unresolved edge from entry to the far jump that returns"
            )
            .to_owned(),
            ..worst("B$ENRA")
        },
    );
}

// Which routines write a runtime cell a program names by EXTDEF, where the
// runtime source settles it. A call to anything else leaves the cell alone,
// unless it runs the program's own code -- which `enters_user_code` and an
// error handler say, and the caller asks.
//
// b$seg, QuickBASIC 4.5: rt/rtinit.asm writes it three times and nothing else
// does. B$DSG0 `MOV [b$seg],DS` and B$DSEG `MOV [b$seg],AX`; B$RTCLR
// `MOV b$seg,DS`, reached through b$clr_disp from clear.asm's B$SCLR (whose
// CLR_RUN B$RUNL and B$StackReset jump to) and through b$run_disp's
// B$RTRUNINI from B$IINIT, B$Init and B$RUNINI. abs.asm, peek.asm and
// bload.asm only read it, and no reference takes its address. ABSOLUTE runs
// machine code of the program's choosing.
pub static WRITERS: LazyLock<IndexMap<(&'static str, &'static str), BTreeSet<&'static str>>> =
    LazyLock::new(|| {
        IndexMap::from_iter([(
            ("b$seg", "qb45"),
            BTreeSet::from([
                "B$DSEG",
                "B$DSG0",
                "B$SCLR",
                "B$RUNL",
                "B$StackReset",
                "B$Init",
                "B$IINIT",
                "B$RUNINI",
                "ABSOLUTE",
            ]),
        )])
    });

/// Whether `name` is a runtime cell only a reference naming it reaches: one
/// `WRITERS` lists, whose address no runtime routine hands out.
pub fn named_only(name: &str, family: &str) -> bool {
    WRITERS.keys().any(|&(cell, of)| cell == name && of == family)
}

/// The per-site map for a whole module, from the object itself.
///
/// One place, because the raise and the lowering must be handed the same
/// answer: built twice from different arguments they can differ.
pub fn for_module(
    found: &Module,
    external: Option<&IndexMap<String, Contract>>,
) -> Result<IndexMap<i64, Contract>, ValueError> {
    let family = module::family(&found.records);
    let mut contracts = per_call(&found.calls, family.value(), &module::defines(&found.records, found.seg));
    contracts.extend(events::contracts(found));
    if family.value() == "vbdos" {
        _zero_entry_sites(found, &mut contracts);
        _redim_sites(found, &mut contracts);
    }
    let inferred = callsite::inferred(found, &contracts);
    contracts.extend(inferred);
    for (name, routine) in external.into_iter().flatten() {
        if routine.name != *name {
            return Err(ValueError(format!(
                "external contract name mismatch: {} != {}",
                pyrepr::string(name),
                pyrepr::string(&routine.name)
            )));
        }
        for (&at, called) in &found.calls {
            if called == name {
                contracts.insert(at, routine.clone());
            }
        }
    }
    Ok(contracts)
}

/// B$ExitDim removes three header words and two bound words per dimension.
pub fn _redim_sites(found: &Module, contracts: &mut IndexMap<i64, Contract>) {
    if !found.calls.values().any(|name| name == "B$RDIM") {
        return;
    }
    let Ok(mapped) = blocks::code_map(found) else {
        return;
    };
    for block in blocks::partition(found, &mapped) {
        for window in block.insns.windows(3) {
            let [rank, descriptor, call] = window else { unreachable!() };
            if found.calls.get(&(call.at as i64)).map(String::as_str) != Some("B$RDIM")
                || rank.end() != descriptor.at
                || descriptor.end() != call.at
            {
                continue;
            }
            if !matches!(rank.insn.code(), Code::Push_imm16 | Code::Pushw_imm8) {
                continue;
            }
            if !matches!(descriptor.insn.code(), Code::Push_imm16 | Code::Pushw_imm8 | Code::Push_r16 | Code::Push_rm16) {
                continue;
            }
            if found.fixup_at.keys().any(|&field| rank.at as i64 <= field && field < rank.end() as i64) {
                continue;
            }
            let dimensions = (rank.insn.immediate(0) & 255) as i64;
            contracts.insert(
                call.at as i64,
                Contract {
                    inputs: Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])),
                    cleanup: Some(6 + 4 * dimensions),
                    evidence: concat!(
                        "VBDCL10E.LIB erase.asm RDIM tails dynamic.asm DIM_COMMON. ",
                        "ExitDim 010a reads [bp+8], clears CH, doubles twice and adds 6; ",
                        "011d..0129 pops the return address, adds that count to SP and jumps back. ",
                        "Rank is an unrelocated immediate word push immediately before the descriptor ",
                        "and call in one basic block. All GP inputs and unknown effects retained."
                    )
                    .to_owned(),
                    ..worst("B$RDIM")
                },
            );
        }
    }
}

/// VBDOS's zero-BX entry bypasses its unresolved helper call.
pub fn _zero_entry_sites(found: &Module, contracts: &mut IndexMap<i64, Contract>) {
    if !found.calls.values().any(|name| name == "B$ENRA") {
        return;
    }
    let Ok(mapped) = blocks::code_map(found) else {
        return;
    };
    for block in blocks::partition(found, &mapped) {
        for window in block.insns.windows(2) {
            let [previous, call] = window else { unreachable!() };
            if found.calls.get(&(call.at as i64)).map(String::as_str) != Some("B$ENRA") || previous.end() != call.at {
                continue;
            }
            let insn = &previous.insn;
            if insn.code() != Code::Mov_r16_imm16 || insn.op0_register() != Register::BX || insn.immediate16() != 0 {
                continue;
            }
            if found.fixup_at.keys().any(|&field| previous.at as i64 <= field && field < previous.end() as i64) {
                continue;
            }
            contracts.insert(
                call.at as i64,
                Contract {
                    inputs: Some(BTreeSet::from([Reg::Bx, Reg::Cx])),
                    cleanup: Some(0),
                    evidence: concat!(
                        "VBDCL10E.LIB rtenexit.asm B$ENRA 0x17..0x55: CX sizes the frame; ",
                        "BX=0 at 0x4b bypasses the helper at 0x5b. A same-block immediate MOV BX,0 ",
                        "immediately precedes this call. All other effects remain worst-case."
                    )
                    .to_owned(),
                    ..worst("B$ENRA")
                },
            );
        }
    }
}

/// One contract per call site, chosen once for the whole module.
///
/// A side map rather than a field on the body: a contract is a fact about
/// the machine, and the raise and the lowering both need the same answer
/// for the same call. Passing it keeps them from looking one up
/// separately and disagreeing -- which is what a per-family contract
/// makes possible, since B$ENRA reads bx under one runtime and not
/// another.
///
/// `family` is `module.Family`'s value, as a string so this stays below
/// module.py. It selects a variant where one is established and changes
/// nothing otherwise -- a caller with no family in hand gets the
/// conservative contract, which is the honest answer for an object whose
/// toolchain nobody read.
///
/// Python's defaults are `family=""` and `defined=frozenset()`.
pub fn per_call(
    calls: &IndexMap<i64, String>,
    family: &str,
    defined: &BTreeSet<String>,
) -> IndexMap<i64, Contract> {
    let variants: &IndexMap<(&str, &str), Contract> = &VARIANTS;
    calls
        .iter()
        .map(|(&at, name)| {
            (
                at,
                if defined.contains(name) {
                    own(name)
                } else {
                    variants
                        .get(&(name.as_str(), family))
                        .cloned()
                        .unwrap_or_else(|| contract(Some(name)))
                },
            )
        })
        .collect()
}

/// Whether this routine's inputs are established at all.
///
/// `inputs` is None where nothing is known and an empty set where the
/// routine is known to read no register. The two are not the same fact
/// and reading them as one is how a call to a routine whose code is not
/// in the tree came out with no requirements at all -- which says it
/// reads nothing, the one thing known to be false about it.
pub fn established_inputs(routine: &Contract) -> bool {
    routine.inputs.is_some()
}

/// Which registers this routine reads its arguments in, in slot order.
///
/// Empty where nothing is established: a routine whose code is not in the
/// tree declares no inputs, and the conservative read set the raise builds
/// for it is a liveness dependency rather than an argument list.
pub fn slots(routine: &Contract) -> Vec<Reg> {
    let Some(inputs) = routine.inputs.as_ref().filter(|inputs| !inputs.is_empty()) else {
        return Vec::new();
    };
    SLOTS
        .into_iter()
        .filter(|one| inputs.contains(one))
        .collect()
}

/// Register values observed by the syntactic continuation of a call.
///
/// ``inputs`` remains the conservative all-path contract used by analyses
/// that must account for hidden runtime transfers. MIR and lowering describe
/// the explicit CFG edge and therefore use this narrower set where one has
/// been established.
pub fn direct_slots(routine: &Contract) -> Vec<Reg> {
    let chosen = if routine.direct_inputs.is_none() {
        &routine.inputs
    } else {
        &routine.direct_inputs
    };
    let Some(chosen) = chosen.as_ref().filter(|chosen| !chosen.is_empty()) else {
        return Vec::new();
    };
    SLOTS
        .into_iter()
        .filter(|one| chosen.contains(one))
        .collect()
}

pub fn writes_caller_memory(routine: &Contract) -> bool {
    routine.writes >= Memory::Strings
}

/// Whether a pass keeping caller values in registers must refuse a body
/// holding this call outright, rather than reasoning about what it touches.
pub fn barrier(routine: &Contract) -> bool {
    routine.enters_user_code || routine.error_handling || !routine.established
}

/// Whether the module has an ON ERROR handler, so a raised error can reach user code.
pub fn handles_errors<'a>(routines: impl IntoIterator<Item = &'a Contract>) -> bool {
    routines.into_iter().any(|routine| routine.error_handling)
}

// helpi4.asm is the model the four shipped contracts in calls.py and stack.py
// were read off, and nothing below contradicts them. cProc's parm declarations
// give the cleanup directly: cEnd emits `ret <parameter bytes>` under the PL/M
// convention (inc/cmacros.inc), and parmD is 4 bytes, parmW and parmSD 2
// (inc/string.inc makes parmSD an alias for parmW).
// All five print entry points in the corpus set ax to a [terminator|value type]
// pair and fall into B$PRINT, which pops its own arguments in a hand-written
// epilogue keyed on that type byte (rt/prnval.asm PRINTX): one word for I2 and
// SD, two for I4. That epilogue also re-pushes the far return address and RETs,
// so control does come back to the byte after the call.
static _PRINT_CLOBBERS: LazyLock<BTreeSet<Reg>> = LazyLock::new(|| {
    EVERY
        .difference(&BTreeSet::from([Reg::Bp, Reg::Si, Reg::Sp]))
        .copied()
        .collect()
});

/// Bytes consumed by value by prnval.asm's PRINTX path, not a descriptor pointer.
pub fn numeric_print_argument(name: &str) -> Option<i64> {
    if !["B$PEI2", "B$PSI2", "B$PEI4", "B$PSI4", "B$PER4"].contains(&name) {
        return None;
    }
    let routine = contract(Some(name));
    if routine.established {
        routine.cleanup
    } else {
        None
    }
}

/// Known scalar stack arguments, whose bits are values rather than caller pointers.
pub fn numeric_stack_arguments(name: &str) -> Option<i64> {
    if ["B$MUI4", "B$DVI4", "B$RMI4", "B$CPI4"].contains(&name) {
        let routine = contract(Some(name));
        return if routine.established && routine.cleanup == Some(8) {
            Some(8)
        } else {
            None
        };
    }
    numeric_print_argument(name)
}

const _PRINT_EVIDENCE: &str = concat!(
    "rt/prnval.asm: the entry point is `MOV AX,<term> SHL 8 + <type>` then `JMP SHORT B$PRINT` (B$PESD falls ",
    "straight in). `cProc B$PRINT,<PUBLIC,FAR>,<SI>` saves si and the epilogue restores si and bp before ",
    "returning, so those two survive; the rest does not, because the numeric path calls B$FOUTBX and the file ",
    "comment there says it `changes all registers except BP` -- flatly contradicting ifout.asm's ",
    "\"Per convention. (DS, ES, SI, DI, BP preserved.)\" for the same routine, and the conservative reading wins. ",
    "PRINTX pops one parameter word, then a second unless `TEST AL,VT_SD` is non-zero, which it is for VT_I2=2 ",
    "and VT_SD=3 and is not for VT_I4=14h (inc/rtps.inc). Memory is the worst case and read as such: print using ",
    "goes through the [b$PUSG] vector into prtu.asm, which calls B$SASS, and output goes through the [VTYP], ",
    "[VWCH] and [b$pFLUSH] vectors whose targets depend on the open device. Any string allocation on those paths ",
    "reaches B$STALC, whose step 4 calls B$STCPCT -- and rt/nhstutil.asm says of it `The string descriptors ",
    "referenced by the string header are adjusted to reflect their movement`, writing [BX+2] of every live ",
    "descriptor including a module-level string variable's. Header: `Exceptions: bad file mode; I/O error or ",
    "Disk full error when flush the buffer if a EOL encountered`."
);

fn _print(name: &str, cleanup: i64) -> Contract {
    Contract {
        name: name.to_owned(),
        // The stub sets ax itself before jumping to B$PRINT, and BC pushes
        // the value: nothing here is a register the caller has to have set.
        inputs: Some(BTreeSet::new()),
        cleanup: Some(cleanup),
        control: Control::Returns,
        enters_user_code: false,
        raises_error: true,
        error_handling: false,
        writes: Memory::Own,
        reads: Memory::Own,
        clobbers: _PRINT_CLOBBERS.clone(),
        established: true,
        evidence: _PRINT_EVIDENCE.to_owned(),
        documented: None,
        direct_inputs: None,
        clobbers_reached: false,
        caller_cleanup: 0,
        i386: false,
        direct_writes: None,
        direct_reads: None,
    }
}

// Everything here reaches user BASIC, or leaves without coming back, or both.
// Their details are recorded for the record; barrier() is the field that
// matters, and it is true for every one of them.
// The three whose own implementation is not in the local tree. B$ENRA and
// B$EXSA are named only by the include files, and B$OGTA only by ulib.inc and
// rtmint.inc -- extent.py's docstring already says so and this does not repeat
// the reasoning. What is established about them is established from the frame
// layout and from measurements of BC's output, not from read code, so
// everything except the fields named in each citation stays at the worst case.
// The x87 helpers, all six of them one module -- 87bhelp.asm -- and byte for
// byte the same object in QuickBASIC 4.5's BCOM45.LIB, PDS 7.1's BCL71ENR.LIB
// and VBDOS 1.0's VBDCL10E.LIB. They are the one group here established from
// a disassembly rather than from source: runtime/inc/rtmint.inc declares them
// and nothing in the 148-file runtime tree defines them, because the math
// library is not in the source drop. tools/libdump.py is what read them out.
//
// Every one of them keeps its own frame (push bp / mov bp,sp ... mov sp,bp /
// pop bp / retf), takes no argument on the 8086 stack -- the operands are in
// registers or already on the x87 stack -- and touches no memory but its own
// scratch below sp. So cleanup is 0 and writes is NONE throughout, and what
// differs between them is only which registers come back changed.
/// One of READ's per-type entries.
///
/// rt/read.asm: `B$RD<type> only sets the type, [b$VTYP], and then jump to
/// a common routine, CommRead`. Entry is `pDest = far pointer to the
/// destination for the data`, so four bytes come off; `cbDest` is a fifth
/// parameter for SD and FS only, and none of those is here.
///
/// It writes through pDest and reads the DATA area, and CommRead reaches
/// B$ReadVal through the [b$GetOneVal] vector -- so Memory.ANY on both
/// sides rather than the destination alone. `Uses: per convention` names
/// nothing preserved. It raises: out of DATA, syntax error, overflow.
fn _read(name: &str) -> Contract {
    Contract {
        name: name.to_owned(),
        // pDest is a parameter, so nothing arrives in a register.
        inputs: Some(BTreeSet::new()),
        cleanup: Some(4),
        control: Control::Returns,
        enters_user_code: false,
        raises_error: true,
        error_handling: false,
        writes: Memory::Own,
        reads: Memory::Own,
        clobbers: EVERY.clone(),
        established: true,
        evidence: concat!(
            "rt/read.asm, the header above B$RDI2: `pDest = far pointer to the destination for the ",
            "data`, `cbDest = for SD and FS only`; `B$RD<type> only sets the type, [b$VTYP], and then ",
            "jump to a common routine, CommRead`. CommRead reaches B$ReadVal through the ",
            "[b$GetOneVal] vector, so what it writes is not bounded by pDest. `Uses: per convention` ",
            "names no preserved register. Exceptions: out of DATA, syntax error, overflow."
        )
        .to_owned(),
        documented: None,
        direct_inputs: None,
        clobbers_reached: false,
        caller_cleanup: 0,
        i386: false,
        direct_writes: None,
        direct_reads: None,
    }
}

// Where the rows live. Data, not code: every field is a claim about the
// runtime, and a claim wants an audit trail more than it wants a Python
// literal. `tools/runtime_writes.py` regenerates the measured columns from
// a linked image, so re-measuring is a diff against this file rather than a
// rewrite of one.
pub const TABLE: &str = include_str!("../../qbopt/abi/runtime.toml");

/// One entry per runtime name the corpus calls, read from the table.
///
/// B$OGTA's control kind comes from blocks.py rather than being restated
/// in the table, which is why this is not simply the rows handed back.
pub fn _contracts(path: Option<&std::path::Path>) -> Result<IndexMap<String, Contract>, String> {
    let text = match path {
        Some(path) => std::fs::read_to_string(path).map_err(|error| error.to_string())?,
        None => TABLE.to_owned(),
    };
    let rows: toml::Table = text
        .parse()
        .map_err(|error: toml::de::Error| error.to_string())?;
    let mut out = IndexMap::default();
    for (name, row) in &rows {
        let row = row
            .as_table()
            .ok_or_else(|| format!("{} is not a table", pyrepr::string(name)))?;
        let field = |key: &str| row.get(key).ok_or_else(|| pyrepr::string(key));
        let integer = |key: &str| {
            field(key)?
                .as_integer()
                .ok_or_else(|| format!("{} is not an int", pyrepr::string(key)))
        };
        let boolean = |key: &str| {
            field(key)?
                .as_bool()
                .ok_or_else(|| format!("{} is not a bool", pyrepr::string(key)))
        };
        let string = |key: &str| {
            field(key)?
                .as_str()
                .ok_or_else(|| format!("{} is not a str", pyrepr::string(key)))
        };
        let regs = |key: &str| -> Result<BTreeSet<Reg>, String> {
            field(key)?
                .as_array()
                .ok_or_else(|| format!("{} is not a list", pyrepr::string(key)))?
                .iter()
                .map(|one| {
                    Reg::from_value(
                        one.as_str()
                            .ok_or_else(|| format!("{one} is not a valid Reg"))?,
                    )
                })
                .collect()
        };
        let one = Contract {
            name: name.clone(),
            cleanup: if integer("cleanup")? < 0 {
                None
            } else {
                Some(integer("cleanup")?)
            },
            control: Control::from_value(string("control")?)?,
            enters_user_code: boolean("enters_user_code")?,
            raises_error: boolean("raises_error")?,
            error_handling: boolean("error_handling")?,
            writes: Memory::from_name(string("writes")?)?,
            reads: Memory::from_name(string("reads")?)?,
            clobbers: regs("clobbers")?,
            established: boolean("established")?,
            evidence: string("evidence")?.trim().to_owned(),
            documented: if row.contains_key("documented") {
                Some(regs("documented")?)
            } else {
                None
            },
            inputs: if row.contains_key("inputs") {
                Some(regs("inputs")?)
            } else {
                None
            },
            direct_inputs: None,
            clobbers_reached: false,
            caller_cleanup: 0,
            i386: false,
            direct_writes: if row.contains_key("direct_writes") {
                Some(Memory::from_name(string("direct_writes")?)?)
            } else {
                None
            },
            direct_reads: if row.contains_key("direct_reads") {
                Some(Memory::from_name(string("direct_reads")?)?)
            } else {
                None
            },
        };
        out.insert(
            name.clone(),
            if INLINE_TABLE.contains(name.as_str()) {
                Contract {
                    control: Control::InlineTable,
                    ..one
                }
            } else {
                one
            },
        );
    }
    Ok(out)
}

pub static CONTRACTS: LazyLock<IndexMap<String, Contract>> =
    LazyLock::new(|| _contracts(None).expect("runtime.toml loads"));

// Routines that hand control back to the program. What they write is what
// the code they call writes, which is anything -- GCC's modref gives up on
// an indirect call for the same reason. B$CENP ends the program, B$EVCK
// polls for an event and may run an event GOSUB, B$OEGA and B$RESN are the
// ON ERROR machinery, and B$FCMD is not established at all.
pub static ENTERS_USER_CODE: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| BTreeSet::from(["B$CENP", "B$EVCK", "B$FCMD", "B$OEGA", "B$RESN"]));

/// Whether a runtime entry never comes back to its caller.
///
/// The table's NEVER rows, plus the error funnel: rt/erproc.asm's RTEDEF
/// macro generates every B$ERR_?? entry as `mov bl,number` falling into
/// B$RUNERR, whose "transfer of control is either handled by environment
/// specific ON ERROR handler, or we fatal error" -- never the caller.
pub fn never_returns(name: &str) -> bool {
    let folded = name.to_uppercase();
    let known = CONTRACTS.get(&folded);
    known.is_some_and(|known| known.established && known.control == Control::Never)
        || folded == "B$RUNERR"
        || folded.starts_with("B$ERR_")
}

// rt/erproc.asm, B$RUNERR's Entry: "[BP] = frame pointer ... [BX] = BASIC
// or internal error number". What follows walks the BP frame chain to an ON
// ERROR handler or B$FERROR; neither reads a register its caller left.
pub static ERROR_FUNNEL_INPUTS: LazyLock<IndexMap<&'static str, BTreeSet<Reg>>> =
    LazyLock::new(|| {
        IndexMap::from_iter([
            ("B$RUNERR", BTreeSet::from([Reg::Bx])),
            ("B$RUNERRINFO", BTreeSet::from([Reg::Bx])),
        ])
    });

/// What this call can do. A name with no entry -- a user SUB or FUNCTION, a
/// runtime routine nothing in the corpus has called yet, an indirect call with
/// no name at all -- gets the worst case rather than an absence.
pub fn contract(name: Option<&str>) -> Contract {
    let Some(name) = name else {
        return worst("");
    };
    CONTRACTS.get(name).cloned().unwrap_or_else(|| worst(name))
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_runtime.py`.
    //!
    //! Skipped, needing `wholeseg`:
    //! `test_registered_handler_does_not_land_in_a_phi_edge`,
    //! `test_handler_discovery_accepts_lowered_push_only_with_matching_relocations`,
    //! `test_addrm_event_adapter_emits_without_fallback`.
    //! `test_event_stub_requires_exact_relocation` monkeypatches `omf.fixups`
    //! to give the field a displacement of 1; here the FIXUPP record itself
    //! says so instead.

    use super::*;
    use crate::objectfile::omf::{self, Record};
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
    }

    fn loaded(name: &str) -> Module {
        module::load(fixture(name)).unwrap().unwrap()
    }

    /// `{0: name}`.
    fn at_zero(name: &str) -> IndexMap<i64, String> {
        IndexMap::from_iter([(0, name.to_owned())])
    }

    /// `runtime.per_call({0: name}, family)[0]`.
    fn one(name: &str, family: &str) -> Contract {
        per_call(&at_zero(name), family, &BTreeSet::new())[&0].clone()
    }

    fn gp() -> BTreeSet<Reg> {
        BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di])
    }

    fn names() -> Vec<String> {
        let mut names: Vec<String> = CONTRACTS.keys().cloned().collect();
        names.sort();
        names
    }

    // qbopt.legacy.calls COMPARE, MULTIPLY, DIVIDE, REMAINDER.
    const ABSORBED: [&str; 4] = ["B$CPI4", "B$MUI4", "B$DVI4", "B$RMI4"];

    const X87: [&str; 6] = ["B$FCMP", "B$FILD", "B$FIL2", "B$FIST", "B$FIS2", "B$FUST"];

    #[test]
    fn enums_repr_like_python() {
        assert_eq!(Reg::Ax.repr(), "<Reg.AX: 'ax'>");
        assert_eq!(Memory::Own.repr(), "<Memory.OWN: 3>");
        assert_eq!(
            Control::InlineTable.repr(),
            "<Control.INLINE_TABLE: 'inline-table'>"
        );
    }

    /// Python's `_contracts()`, one `name repr` line each, frozensets sorted.
    #[test]
    fn the_contract_table_matches_python() {
        let mut got: Vec<String> = _contracts(None)
            .unwrap()
            .iter()
            .map(|(name, one)| format!("{name} {}", one.repr()))
            .collect();
        got.sort();
        let want: Vec<&str> = include_str!("../../fixtures/abi/runtime-contracts.txt")
            .lines()
            .collect();
        assert_eq!(got, want);
        let order: Vec<&str> = include_str!("../../fixtures/abi/runtime-contract-order.txt")
            .lines()
            .collect();
        assert_eq!(CONTRACTS.keys().collect::<Vec<_>>(), order);
    }

    #[test]
    fn the_variants_match_python() {
        let mut got: Vec<String> = VARIANTS
            .iter()
            .map(|((name, family), one)| format!("{name} {family} {}", one.repr()))
            .collect();
        got.sort();
        let want: Vec<&str> = include_str!("../../fixtures/abi/runtime-variants.txt")
            .lines()
            .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn test_unknown_name_is_the_worst_case() {
        let unknown = contract(Some("B$NOSUCHTHING"));
        assert_eq!(unknown, worst("B$NOSUCHTHING"));
        assert!(!unknown.established);
        assert!(barrier(&unknown));
    }

    #[test]
    fn test_an_unnamed_call_is_the_worst_case() {
        assert_eq!(contract(None).clobbers, *EVERY);
    }

    #[test]
    fn test_evk1_alias_keeps_event_effects() {
        for family in ["pds71", "vbdos"] {
            let alias = one("B$EVK1", family);
            let original = contract(Some("B$EVCK"));
            assert_eq!(
                Contract {
                    name: original.name.clone(),
                    evidence: original.evidence.clone(),
                    ..alias.clone()
                },
                original
            );
            assert!(barrier(&alias));
            assert!(alias.enters_user_code);
            assert_eq!(alias.writes, Memory::Any);
        }
    }

    #[test]
    fn test_evk1_alias_requires_an_established_family() {
        for family in ["", "qb45", "unknown"] {
            assert_eq!(one("B$EVK1", family), worst("B$EVK1"));
        }
    }

    #[test]
    fn test_evk1_user_definition_overrides_runtime_alias() {
        let defined = BTreeSet::from(["B$EVK1".to_owned()]);
        assert_eq!(
            per_call(&at_zero("B$EVK1"), "pds71", &defined)[&0],
            own("B$EVK1")
        );
    }

    #[test]
    fn test_timer_interfaces_bound_inputs_without_claiming_preservation() {
        for family in ["pds71", "vbdos"] {
            for name in ["B$ONTA", "B$ETT0", "B$ETT1", "B$ETT2"] {
                let routine = one(name, family);
                assert_eq!(routine.inputs, Some(gp()));
                assert_eq!(
                    Contract {
                        inputs: None,
                        evidence: worst(name).evidence,
                        ..routine
                    },
                    worst(name)
                );
                assert_eq!(one(name, ""), worst(name));
            }
        }
    }

    #[test]
    fn test_runtime_return_interface_keeps_unknown_effects() {
        for family in ["pds71", "vbdos"] {
            let routine = one("B$RETA", family);
            assert_eq!(routine.inputs, Some(gp()));
            assert_eq!(
                Contract {
                    inputs: None,
                    evidence: worst("B$RETA").evidence,
                    ..routine
                },
                worst("B$RETA")
            );
        }
    }

    #[test]
    fn test_command_line_has_bounded_inputs_without_optimistic_effects() {
        for family in ["qb45", "pds71", "vbdos"] {
            let routine = one("B$FCMD", family);
            assert_eq!(routine.inputs, Some(gp()));
            assert_eq!(routine.cleanup, Some(0));
            assert_eq!(
                Contract {
                    inputs: None,
                    cleanup: None,
                    evidence: worst("B$FCMD").evidence,
                    ..routine
                },
                worst("B$FCMD")
            );
            assert_eq!(one("B$FCMD", "").inputs, None);
        }
    }

    #[test]
    fn test_the_worst_case_concedes_nothing() {
        let blank = worst("");
        assert_eq!(blank.writes, Memory::Any);
        assert_eq!(blank.reads, Memory::Any);
        assert_eq!(blank.clobbers, *EVERY);
        assert_eq!(blank.control, Control::Unknown);
        assert_eq!(blank.cleanup, None);
        assert!(blank.enters_user_code);
        assert!(writes_caller_memory(&blank));
        assert_eq!(preserves(&blank), BTreeSet::new());
    }

    #[test]
    fn test_no_contract_claims_more_than_the_worst_case() {
        for name in names() {
            let routine = &CONTRACTS[&name];
            let blank = worst(&routine.name);
            assert!(routine.writes <= blank.writes);
            assert!(routine.reads <= blank.reads);
            assert!(routine.clobbers.is_subset(&blank.clobbers));
            assert!(routine.enters_user_code <= blank.enters_user_code);
        }
    }

    #[test]
    fn test_an_unestablished_routine_is_left_at_the_worst_case() {
        for name in names()
            .into_iter()
            .filter(|name| !CONTRACTS[name].established)
        {
            let routine = &CONTRACTS[&name];
            assert_eq!(
                *routine,
                Contract {
                    evidence: routine.evidence.clone(),
                    ..worst(&name)
                }
            );
        }
    }

    #[test]
    fn test_every_claim_below_the_worst_case_cites_something() {
        for name in names()
            .into_iter()
            .filter(|name| CONTRACTS[name] != worst(name))
        {
            let evidence = &CONTRACTS[&name].evidence;
            assert!(
                [".asm", ".inc", ".py", "AGENTS.md"]
                    .iter()
                    .any(|cited| evidence.contains(cited))
            );
        }
    }

    #[test]
    fn test_preserved_and_clobbered_partition_the_registers() {
        for name in names() {
            let routine = &CONTRACTS[&name];
            assert_eq!(
                preserves(routine)
                    .union(&routine.clobbers)
                    .copied()
                    .collect::<BTreeSet<_>>(),
                *EVERY
            );
            assert!(preserves(routine).is_disjoint(&routine.clobbers));
        }
    }

    #[test]
    fn test_a_documented_clobber_set_is_recorded_only_where_it_differs() {
        for name in names()
            .into_iter()
            .filter(|name| CONTRACTS[name].documented.is_some())
        {
            let routine = &CONTRACTS[&name];
            let mut clobbers = routine.clobbers.clone();
            clobbers.remove(&Reg::Flags);
            assert_ne!(routine.documented, Some(clobbers));
        }
    }

    #[test]
    fn test_the_table_is_keyed_by_its_own_names() {
        for name in names() {
            assert_eq!(CONTRACTS[&CONTRACTS[&name].name], CONTRACTS[&name]);
        }
    }

    #[test]
    fn test_the_inline_table_routines_come_from_blocks() {
        let reflected: BTreeSet<&str> = CONTRACTS
            .iter()
            .filter(|(_, routine)| routine.control == Control::InlineTable)
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(reflected, *INLINE_TABLE);
    }

    #[test]
    fn test_anything_touching_user_code_is_refused() {
        for name in ["B$EVCK", "B$OEGA", "B$RESN", "B$FERR"] {
            assert!(barrier(&CONTRACTS[name]));
        }
    }

    #[test]
    fn test_the_dispatchers_can_do_anything() {
        for name in ["B$EVCK", "B$OEGA", "B$RESN"] {
            let dispatcher = &CONTRACTS[name];
            assert!(dispatcher.enters_user_code);
            assert_eq!(dispatcher.writes, Memory::Any);
            assert_eq!(dispatcher.reads, Memory::Any);
            assert_eq!(dispatcher.clobbers, *EVERY);
        }
    }

    #[test]
    fn test_resume_separates_its_direct_footprint_from_resumed_user_code() {
        let resume = &CONTRACTS["B$RESN"];
        assert!(resume.reads == Memory::Any && resume.writes == Memory::Any);
        assert_eq!(resume.direct_reads, Some(Memory::Own));
        assert_eq!(resume.direct_writes, Some(Memory::Strings));
    }

    #[test]
    fn test_the_err_function_refuses_the_body_without_claiming_to_dispatch() {
        let err = &CONTRACTS["B$FERR"];
        assert!(!err.enters_user_code);
        assert!(err.error_handling);
        assert!(barrier(err));
    }

    #[test]
    fn test_the_routines_that_do_not_come_back_say_so() {
        for name in ["B$CENP", "B$CEND", "B$RESN"] {
            assert_eq!(CONTRACTS[name].control, Control::Never);
        }
    }

    #[test]
    fn test_cleanup_agrees_with_the_arity_calls_py_measured() {
        for name in ABSORBED {
            assert_eq!(CONTRACTS[name].cleanup, Some(8));
        }
    }

    #[test]
    fn test_the_absorbed_four_touch_no_caller_memory() {
        for name in ABSORBED {
            let absorbed = &CONTRACTS[name];
            assert!(!writes_caller_memory(absorbed));
            assert!(absorbed.reads <= Memory::Arguments);
            assert_eq!(absorbed.control, Control::Returns);
            assert!(!absorbed.enters_user_code);
            assert!(absorbed.clobbers.is_subset(&BTreeSet::from([
                Reg::Ax,
                Reg::Bx,
                Reg::Cx,
                Reg::Dx,
                Reg::Flags
            ])));
        }
    }

    #[test]
    fn test_compare_clobbers_only_the_flags_it_returns_in() {
        let compare = &CONTRACTS["B$CPI4"];
        assert_eq!(compare.clobbers, BTreeSet::from([Reg::Flags]));
        assert_eq!(
            compare.documented,
            Some(BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx]))
        );
    }

    #[test]
    fn test_the_x87_helpers_keep_the_index_registers() {
        for name in X87 {
            let kept = preserves(&CONTRACTS[name]);
            assert!(BTreeSet::from([Reg::Si, Reg::Di, Reg::Bx, Reg::Cx]).is_subset(&kept));
        }
    }

    #[test]
    fn test_the_x87_helpers_touch_no_caller_memory() {
        for name in X87 {
            assert!(!writes_caller_memory(&CONTRACTS[name]));
            assert!(!barrier(&CONTRACTS[name]));
        }
    }

    #[test]
    fn test_the_two_forms_differ_only_by_the_sign_extension() {
        let long_form = &CONTRACTS["B$FILD"];
        let int_form = &CONTRACTS["B$FIL2"];
        assert!(!long_form.clobbers.contains(&Reg::Dx));
        assert!(int_form.clobbers.contains(&Reg::Dx));
        let mut without = int_form.clobbers.clone();
        without.remove(&Reg::Dx);
        assert_eq!(without, long_form.clobbers);
    }

    #[test]
    fn test_the_result_registers_are_the_clobbered_ones() {
        assert!(CONTRACTS["B$FIST"].clobbers.contains(&Reg::Ax));
        assert!(CONTRACTS["B$FIST"].clobbers.contains(&Reg::Dx));
        for name in ["B$FIS2", "B$FUST"] {
            assert!(CONTRACTS[name].clobbers.contains(&Reg::Ax));
            assert!(
                !CONTRACTS[name].clobbers.contains(&Reg::Dx),
                "{name} returns one word"
            );
        }
    }

    #[test]
    fn test_the_string_routines_write_caller_memory() {
        for name in ["B$SASS", "B$STDL", "B$SCAT", "B$STI2", "B$LTRM"] {
            assert!(writes_caller_memory(&CONTRACTS[name]));
        }
    }

    #[test]
    fn test_the_routines_that_touch_no_caller_memory() {
        for name in ABSORBED.into_iter().chain(["B$DSEG", "B$FERR"]) {
            assert!(!writes_caller_memory(&CONTRACTS[name]));
        }
    }

    #[test]
    fn test_a_contract_says_which_registers_it_reads() {
        assert_eq!(
            contract(Some("B$FILD")).inputs,
            Some(BTreeSet::from([Reg::Ax, Reg::Dx]))
        );
        assert_eq!(
            contract(Some("B$FIL2")).inputs,
            Some(BTreeSet::from([Reg::Ax]))
        );
        assert_eq!(contract(Some("B$FIST")).inputs, Some(BTreeSet::new()));
        assert!(!contract(Some("B$NOSUCH")).established);
    }

    #[test]
    fn test_on_goto_takes_its_branch_index_in_bx() {
        assert_eq!(
            contract(Some("B$OGTA")).inputs,
            Some(BTreeSet::from([Reg::Bx]))
        );
    }

    #[test]
    fn test_every_established_contract_says_what_it_reads() {
        let mut unknown: Vec<&str> = CONTRACTS
            .iter()
            .filter(|(_, one)| one.established && one.inputs.is_none())
            .map(|(name, _)| name.as_str())
            .collect();
        unknown.sort();
        assert_eq!(
            unknown,
            ["B$ENRA", "B$EXSA"],
            "unestablished inputs: {unknown:?}"
        );
        assert_eq!(
            worst("anything").inputs,
            None,
            "a name with no entry reads everything"
        );
    }

    #[test]
    fn test_a_routine_that_enters_user_code_still_writes_anything() {
        for name in ["B$CENP", "B$EVCK", "B$OEGA", "B$RESN", "B$FCMD"] {
            let contract = contract(Some(name));
            assert!(
                contract.enters_user_code,
                "{name} is listed here but does not enter user code"
            );
            assert_eq!(
                contract.writes,
                Memory::Any,
                "{name} was narrowed and must not be"
            );
        }
        assert_eq!(
            contract(Some("B$NOTAROUTINE")).writes,
            Memory::Any,
            "an unestablished contract must concede everything"
        );
    }

    #[test]
    fn test_the_measured_routines_are_narrowed() {
        let narrowed: Vec<&String> = CONTRACTS
            .iter()
            .filter(|(_, one)| one.writes == Memory::Own)
            .map(|(name, _)| name)
            .collect();
        assert!(
            narrowed.len() >= 15,
            "only {} routines carry the measurement",
            narrowed.len()
        );
        for name in narrowed {
            assert!(
                !CONTRACTS[name].enters_user_code,
                "{name} enters user code and is narrowed"
            );
        }
    }

    #[test]
    fn test_every_row_in_the_table_is_loaded() {
        let rows: toml::Table = TABLE.parse().unwrap();
        assert_eq!(
            rows.keys().collect::<BTreeSet<_>>(),
            CONTRACTS.keys().collect::<BTreeSet<_>>(),
            "the table and what loaded disagree"
        );
        for name in ["B$HARY", "B$LINA"] {
            for family in ["qb45", "pds71", "vbdos"] {
                assert_eq!(VARIANTS[&(name, family)], CONTRACTS[name]);
            }
        }
        for (name, row) in &rows {
            let one = &CONTRACTS[name];
            assert_eq!(one.writes.name(), row["writes"].as_str().unwrap());
            assert_eq!(one.reads.name(), row["reads"].as_str().unwrap());
            assert_eq!(
                one.clobbers
                    .iter()
                    .map(|r| r.value())
                    .collect::<BTreeSet<_>>(),
                row["clobbers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r.as_str().unwrap())
                    .collect::<BTreeSet<_>>()
            );
            assert_eq!(one.established, row["established"].as_bool().unwrap());
            assert_eq!(
                one.evidence.trim(),
                row["evidence"].as_str().unwrap().trim()
            );
        }
    }

    #[test]
    fn test_an_unestablished_row_concedes_everything() {
        for (name, one) in CONTRACTS.iter() {
            if one.established {
                continue;
            }
            assert_eq!(
                one.writes,
                Memory::Any,
                "{name} is not established and narrows its writes"
            );
            assert_eq!(
                one.reads,
                Memory::Any,
                "{name} is not established and narrows its reads"
            );
        }
    }

    #[test]
    fn test_the_semicolon_long_print_reads_no_register() {
        let one = contract(Some("B$PSI4"));
        assert!(one.established, "B$PSI4 is not in the table");
        assert_eq!(
            one.inputs,
            Some(BTreeSet::new()),
            "it reads {:?}",
            one.inputs
        );
        assert_eq!(one.cleanup, Some(4), "it pops {:?}", one.cleanup);
        assert_eq!(one.cleanup, contract(Some("B$PEI4")).cleanup);
        assert_eq!(one.clobbers, contract(Some("B$PSI2")).clobbers);
        assert_eq!(one.writes, contract(Some("B$PSI2")).writes);
        assert_eq!(one.reads, contract(Some("B$PSI2")).reads);
    }

    // tests/test_environ_contract.py

    #[test]
    fn test_vbdos_environ_keeps_general_inputs_and_unknown_effects() {
        let rule = one("B$FEVS", "vbdos");
        assert_eq!(rule.inputs, Some(gp()));
        assert_eq!(rule.cleanup, None);
        assert_eq!(rule.clobbers, *EVERY);
        assert!(rule.reads == Memory::Any && rule.writes == Memory::Any);
        assert_eq!(rule.control, Control::Unknown);
        assert!(rule.raises_error && barrier(&rule));
    }

    #[test]
    fn test_environ_evidence_does_not_claim_other_runtime_versions() {
        for family in ["qb45", "pds71", ""] {
            assert_eq!(one("B$FEVS", family).inputs, None);
        }
    }

    // tests/test_trig_contract.py

    #[test]
    fn test_vbdos_trig_retains_conservative_register_and_memory_effects() {
        for name in ["B$SIN4", "B$SIN8", "B$COS4", "B$COS8"] {
            let routine = one(name, "vbdos");
            assert_eq!(routine.inputs, Some(gp()));
            assert_eq!(routine.cleanup, Some(0));
            assert_eq!(routine.clobbers, *EVERY);
            assert_eq!(routine.reads, Memory::Any);
            assert_eq!(routine.writes, Memory::Any);
            assert!(routine.raises_error);
        }
    }

    // tests/test_rounding_contracts.py

    #[test]
    fn test_vbdos_string_conversion_keeps_unproved_effects() {
        for symbol in ["B$STR4", "B$STR8"] {
            let rule = one(symbol, "vbdos");
            assert_eq!(rule.inputs, Some(gp()));
            assert_eq!(rule.cleanup, None);
            assert_eq!(rule.clobbers, *EVERY);
            assert!(rule.reads == Memory::Any && rule.writes == Memory::Any);
            assert!(rule.control == Control::Unknown && rule.raises_error);
            assert_eq!(one(symbol, "qb45").inputs, None);
        }
    }

    #[test]
    fn test_vbdos_rounding_keeps_unknown_effects() {
        for symbol in ["B$INT4", "B$INT8"] {
            let rule = one(symbol, "vbdos");
            assert_eq!(rule.cleanup, Some(0));
            assert_eq!(rule.inputs, Some(gp()));
            assert_eq!(rule.clobbers, *EVERY);
            assert!(rule.reads == Memory::Any && rule.writes == Memory::Any);
            assert!(rule.control == Control::Unknown && rule.raises_error);
            assert_eq!(one(symbol, "qb45").inputs, None);
        }
    }

    // tests/test_file_contracts.py

    #[test]
    fn test_peos_register_interface_does_not_claim_fixed_stack_cleanup() {
        let contract = one("B$PEOS", "vbdos");
        assert_eq!(contract.inputs, Some(gp()));
        assert_eq!(contract.cleanup, None);
        assert_eq!(contract.clobbers, *EVERY);
        assert_eq!(contract.control, Control::Unknown);
        assert_eq!(contract.writes, Memory::Any);
    }

    #[test]
    fn test_vbdos_file_setup_retains_unknown_effects() {
        for (name, cleanup) in [
            ("B$FREF", Some(0)),
            ("B$LDFS", Some(6)),
            ("B$OPEN", Some(8)),
            ("B$DSKI", Some(2)),
            ("B$FEOF", Some(2)),
            ("B$CLOS", None),
            ("B$ERAS", Some(2)),
            ("B$FLEN", Some(2)),
            ("B$FMID", Some(6)),
            ("B$ASSN", Some(12)),
            ("B$SCMP", Some(4)),
            ("B$SCPF", Some(2)),
            ("B$LNIN", Some(10)),
            ("B$ERS1", Some(2)),
            ("B$RTRM", Some(2)),
            ("B$FASC", Some(2)),
            ("B$FCHR", Some(2)),
            ("B$LEFT", Some(4)),
            ("B$RGHT", Some(4)),
            ("B$FMKI", Some(2)),
            ("B$FMKL", Some(4)),
            ("B$FCVI", Some(2)),
            ("B$FCVS", Some(2)),
            ("B$CSCN", None),
            ("B$WIDT", Some(4)),
            ("B$SLEP", Some(4)),
            ("B$TIMR", Some(0)),
            ("B$FRI2", Some(2)),
            ("B$STI4", Some(4)),
        ] {
            let contract = one(name, "vbdos");
            assert_eq!(contract.cleanup, cleanup, "{name}");
            assert_eq!(contract.inputs, Some(gp()));
            assert_eq!(contract.clobbers, *EVERY);
            assert_eq!(contract.reads, Memory::Any);
            assert_eq!(contract.writes, Memory::Any);
            assert_eq!(contract.control, Control::Unknown);
            assert!(contract.raises_error);
        }
    }

    #[test]
    fn test_vbdos_erase_has_no_direct_register_operand() {
        let rule = one("B$ERAS", "vbdos");
        assert_eq!(rule.inputs, Some(gp()));
        assert_eq!(direct_slots(&rule), Vec::new());
    }

    #[test]
    fn test_qb45_string_assignment_and_double_print_have_fixed_cleanup() {
        for (name, cleanup) in [("B$ASSN", 12), ("B$PER8", 8)] {
            assert_eq!(one(name, "qb45").cleanup, Some(cleanup));
        }
    }

    // tests/test_entry_contract.py

    #[test]
    fn test_vbdos_output_channel_keeps_unknown_effects() {
        let contract = one("B$CHOU", "vbdos");
        assert_eq!(contract.inputs, Some(gp()));
        assert_eq!(contract.cleanup, Some(2));
        assert_eq!(contract.clobbers, *EVERY);
        assert_eq!(contract.reads, Memory::Any);
        assert_eq!(contract.writes, Memory::Any);
        assert_eq!(contract.control, Control::Unknown);
        assert!(contract.raises_error);
    }

    #[test]
    fn test_vbdos_statement_exit_has_stack_neutral_interface() {
        let contract = one("B$EXTS", "vbdos");
        assert_eq!(contract.inputs, Some(gp()));
        assert_eq!(contract.direct_inputs, Some(BTreeSet::new()));
        assert_eq!(contract.cleanup, Some(0));
        assert_eq!(contract.clobbers, *EVERY);
        assert_eq!(contract.reads, Memory::Any);
        assert_eq!(contract.writes, Memory::Any);
        assert_eq!(contract.control, Control::Unknown);
        assert!(contract.raises_error);
    }

    #[test]
    fn test_vbdos_fixed_udt_assignment_reads_only_its_stack_arguments() {
        assert_eq!(one("B$ASSN", "vbdos").direct_inputs, Some(BTreeSet::new()));
    }

    #[test]
    fn test_vbdos_float_print_interfaces() {
        for (name, cleanup) in [
            ("B$PCR4", 4),
            ("B$PSR4", 4),
            ("B$PCR8", 8),
            ("B$PSR8", 8),
            ("B$PER8", 8),
        ] {
            let contract = one(name, "vbdos");
            assert_eq!(contract.cleanup, Some(cleanup));
            assert_eq!(contract.inputs, Some(gp()));
            assert_eq!(contract.clobbers, *EVERY);
            assert_eq!(contract.reads, Memory::Any);
            assert_eq!(contract.writes, Memory::Any);
            assert_eq!(contract.control, Control::Unknown);
            assert!(contract.raises_error);
        }
    }

    #[test]
    fn test_event_stub_near_call_has_no_register_arguments() {
        // ADDRM /V refused at 0048 before its first statement could execute.
        for tag in ["p-evt", "v-evt"] {
            let found = loaded(&format!("fixtures/omf/addrm-{tag}.obj"));
            let routine = for_module(&found, None).unwrap()[&0x48].clone();
            assert_eq!(routine.inputs, Some(BTreeSet::new()), "{tag}");
            assert_eq!(routine.cleanup, Some(0), "{tag}");
            assert!(routine.enters_user_code && barrier(&routine), "{tag}");
            assert!(routine.reads == Memory::Any && routine.writes == Memory::Any, "{tag}");
        }
    }

    #[test]
    fn test_changed_event_stub_remains_unknown() {
        // Only instruction bytes: a relocated field's addend is folded into its fixup before recognition.
        let found = loaded("fixtures/omf/addrm-p-evt.obj");
        let width = |loc: i64| match loc {
            omf::LOC_OFF16 => 2,
            omf::LOC_PTR32 => 4,
            _ => 2,
        };
        let relocated: BTreeSet<i64> = omf::fixups(&found.records)
            .into_iter()
            .filter(|one| one.seg == Some(found.seg))
            .flat_map(|one| one.offset..one.offset + width(one.loc))
            .collect();
        for at in (0x30..0x42).filter(|at| !relocated.contains(at)) {
            let mut changed = found.clone();
            changed.code[at as usize] ^= 1;
            assert!(!for_module(&changed, None).unwrap().contains_key(&0x48), "{at:#x}");
        }
    }

    #[test]
    fn test_event_stub_requires_exact_relocation() {
        let found = loaded("fixtures/omf/addrm-p-evt.obj");
        for field in [0x34, 0x3E] {
            let fixup = omf::fixups(&found.records)
                .into_iter()
                .find(|one| one.seg == Some(found.seg) && one.offset == field)
                .unwrap();
            // Give the fixup an explicit displacement of 1, adding the field
            // where the subrecord had none.
            let mut body = fixup.record.body.clone();
            match fixup.disp_pos {
                Some(at) => body[at..at + 2].copy_from_slice(&1u16.to_le_bytes()),
                None => {
                    let mut sub = body[fixup.lo..fixup.hi].to_vec();
                    sub[2] &= !0x04;
                    sub.extend_from_slice(&1u16.to_le_bytes());
                    body.splice(fixup.lo..fixup.hi, sub);
                }
            }
            let edited = Rc::new(Record { r#type: fixup.record.r#type, body, raw: None });
            let mut changed = found.clone();
            changed.records = found
                .records
                .iter()
                .map(|one| if Rc::ptr_eq(one, &fixup.record) { edited.clone() } else { one.clone() })
                .collect();
            let disp = omf::fixups(&changed.records)
                .into_iter()
                .find(|one| one.seg == Some(found.seg) && one.offset == field)
                .map(|one| one.disp);
            assert_eq!(disp, Some(1), "{field:#x}");
            assert!(!for_module(&changed, None).unwrap().contains_key(&0x48), "{field:#x}");
        }
    }

    /// Every `B$` routine a committed OMF fixture calls.
    fn _runtime_targets() -> BTreeSet<String> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(fixture("fixtures/omf"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "obj"))
            .collect();
        paths.sort();
        let mut named = BTreeSet::new();
        for path in paths {
            let records = omf::parse(&std::fs::read(&path).unwrap()).unwrap();
            if let Some(found) = module::of(&records) {
                named.extend(found.calls.values().filter(|name| name.starts_with("B$")).cloned());
            }
        }
        named
    }

    #[test]
    fn test_every_runtime_routine_the_corpus_calls_has_an_entry() {
        let names = _runtime_targets();
        assert!(!names.is_empty());
        for name in names {
            assert!(contract(Some(&name)).established, "{name}");
        }
    }
}
