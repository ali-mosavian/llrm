//! Which 64-bit and counting operations a size-built module calls the runtime
//! for at the 68020 and later, by the bytes of the whole linked program.
//!
//! # How it works
//! [`chosen`] counts the sites of each [`Kind`] in the functions built for
//! size, asks [`llrm_codegen::backend::libcall_choice::choose`] with the
//! costs in `libcalls.tsv`, and [`apply`] sets `LibCall` (and `Expand`, for the
//! shift parts) for the kinds it returns. One use of a 64-bit count is
//! cheaper inline; a second makes the call cheaper, the routine being paid
//! once. A function built for speed never calls: the library is slower
//! (bitcount_u64 -O2: +8% clocks at the 68020, +18% at the 68060).
//!
//! DEVIATE M68K-SIZE-LIBCALL: see [`llrm_codegen::backend::libcall_choice`].
//! The 68000 and 68010 call these always (BC1, `dagisel/lowering.rs`). Owner
//! m68k. Review: 2026-12-20.

use std::collections::HashMap;
use std::sync::LazyLock;

use llrm_codegen::backend::libcall_choice::{Candidate, Costs, Member, choose};
use llrm_codegen::backend::selectiondag::actions::{Action, TargetLowering};
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::MVT;
use llrm_mir::context::ConstantKind;
use llrm_mir::intrinsics::{BitCount, Intrinsic};
use llrm_mir::module::{Function, Module, Operand};
use llrm_mir::opcode::{BinaryOp, Opcode};

/// How a build chooses: by bytes (the default), or one way for everything
/// (`-fsize-libcalls=inline|call`, which the size table is measured with).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Auto,
    Inline,
    Call,
}

impl Mode {
    pub fn parse(text: &str) -> Option<Mode> {
        match text {
            "auto" => Some(Mode::Auto),
            "inline" => Some(Mode::Inline),
            "call" => Some(Mode::Call),
            _ => None,
        }
    }
}

/// An operation that can be inline or a call.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    Ctz64,
    Clz64,
    Pop32,
    Pop64,
    Shl64,
    Shr64,
    Sar64,
}

const KINDS: [Kind; 7] = [Kind::Ctz64, Kind::Clz64, Kind::Pop32, Kind::Pop64, Kind::Shl64, Kind::Shr64, Kind::Sar64];

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Ctz64 => "Ctz64",
            Kind::Clz64 => "Clz64",
            Kind::Pop32 => "Pop32",
            Kind::Pop64 => "Pop64",
            Kind::Shl64 => "Shl64",
            Kind::Shr64 => "Shr64",
            Kind::Sar64 => "Sar64",
        }
    }

    /// The actions that make this kind a call: the count's own, or the shift
    /// parts left to the legalizer, which then calls the routine (a
    /// shift by a constant stays inline).
    fn actions(self) -> &'static [(ISD, MVT, Action)] {
        match self {
            Kind::Ctz64 => &[(ISD::CtTzZeroPoison, MVT::I64, Action::LibCall)],
            Kind::Clz64 => &[(ISD::CtLzZeroPoison, MVT::I64, Action::LibCall)],
            Kind::Pop32 => &[(ISD::CtPop, MVT::I32, Action::LibCall)],
            Kind::Pop64 => &[(ISD::CtPop, MVT::I64, Action::LibCall)],
            Kind::Shl64 => &[(ISD::ShlParts, MVT::I32, Action::Expand)],
            Kind::Shr64 => &[(ISD::SrlParts, MVT::I32, Action::Expand)],
            Kind::Sar64 => &[(ISD::SraParts, MVT::I32, Action::Expand)],
        }
    }
}

static COSTS: LazyLock<Costs<Kind>> = LazyLock::new(|| parse(include_str!("libcalls.tsv")));

/// The table: `candidate KEY ROUTINE INLINE CALL` and `member ROUTINE BYTES
/// [NEEDS,..]` lines, `#` comments.
pub fn parse(text: &str) -> Costs<Kind> {
    let mut costs = Costs { candidates: Vec::new(), members: HashMap::new() };
    for line in text.lines().filter(|line| !line.starts_with('#') && !line.trim().is_empty()) {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields[..] {
            ["candidate", key, routine, inline_bytes, call_bytes] => costs.candidates.push(Candidate {
                key: KINDS
                    .into_iter()
                    .find(|kind| kind.name() == key)
                    .unwrap_or_else(|| panic!("libcalls.tsv: kind {key}")),
                routine: routine.to_owned(),
                inline_bytes: inline_bytes.parse().expect("inline bytes"),
                call_bytes: call_bytes.parse().expect("call bytes"),
            }),
            ["member", routine, bytes] => {
                costs
                    .members
                    .insert(
                        routine.to_owned(),
                        Member { bytes: bytes.parse().expect("member bytes"), needs: Vec::new() },
                    );
            }
            ["member", routine, bytes, needs] => {
                costs
                    .members
                    .insert(
                        routine.to_owned(),
                        Member {
                            bytes: bytes.parse().expect("member bytes"),
                            needs: needs.split(',').map(str::to_owned).collect(),
                        },
                    );
            }
            _ => panic!("libcalls.tsv: {line}"),
        }
    }
    costs
}

/// The table's costs.
pub fn costs() -> &'static Costs<Kind> {
    &COSTS
}

/// The sites of each kind in `function`.
fn count(
    module: &Module,
    function: &Function,
    sites: &mut HashMap<Kind, u32>,
) {
    let types = &module.context.types;
    for (_, inst) in function.walk() {
        let instruction = function.instruction(inst);
        let bits = |operand: Operand| function.operand_type(&module.context, operand).and_then(|ty| types.int_bits(ty));
        let kind = match &instruction.opcode {
            Opcode::Call(_) => {
                let Some(Operand::Constant(id)) = instruction.operands.last().copied() else { continue };
                let ConstantKind::Global(global) = module.context.get(id).kind else { continue };
                let Some(Intrinsic::BitCount(count)) = module.global(global).intrinsic() else { continue };
                match (count, bits(instruction.operands[0])) {
                    (BitCount::TrailingZeros, Some(64)) => Kind::Ctz64,
                    (BitCount::LeadingZeros, Some(64)) => Kind::Clz64,
                    (BitCount::Population, Some(32)) => Kind::Pop32,
                    (BitCount::Population, Some(64)) => Kind::Pop64,
                    _ => continue,
                }
            }
            // a shift by a constant is expanded in line whatever the choice
            Opcode::Binary(op @ (BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr))
                if bits(instruction.operands[0]) == Some(64)
                    && !matches!(instruction.operands[1], Operand::Constant(_)) =>
            {
                match op {
                    BinaryOp::Shl => Kind::Shl64,
                    BinaryOp::LShr => Kind::Shr64,
                    _ => Kind::Sar64,
                }
            }
            _ => continue,
        };
        *sites.entry(kind).or_default() += 1;
    }
}

/// The kinds to call in this module's size-built functions (`built_for_size`
/// says which), in `mode`.
pub fn chosen(
    module: &Module,
    mode: Mode,
    built_for_size: impl Fn(&Function) -> bool,
) -> Vec<Kind> {
    let mut sites = HashMap::new();
    for (_, _, function) in module.functions().filter(|(_, _, one)| !one.is_declaration()) {
        if built_for_size(function) {
            count(module, function, &mut sites);
        }
    }
    let mut kinds = match mode {
        Mode::Auto => choose(&COSTS, &sites),
        Mode::Inline => Vec::new(),
        Mode::Call => KINDS.to_vec(),
    };
    // the unit that defines a routine is its body: a call would recurse
    kinds.retain(|kind| {
        let routine = COSTS.candidates.iter().find(|one| one.key == *kind).map(|one| one.routine.as_str());
        !routine
            .and_then(|name| module.named(name))
            .is_some_and(
                |id| matches!(
                    &module.global(id).kind,
                    llrm_mir::GlobalKind::Function(function) if !function.is_declaration()
                ),
            )
    });
    kinds
}

/// Makes `kinds` calls in `tli`.
pub fn apply(
    tli: &mut TargetLowering,
    kinds: &[Kind],
) {
    for kind in kinds {
        for &(op, vt, action) in kind.actions() {
            tli.set_operation_action(op, vt, action);
        }
    }
}

/// `kinds` as a bit set, for [`crate::compile::Options`] (which is `Copy`).
pub fn mask(kinds: &[Kind]) -> u8 {
    kinds.iter().fold(0, |bits, kind| bits | 1 << KINDS.iter().position(|one| one == kind).expect("a kind"))
}

/// The kinds of a [`mask`].
pub fn from_mask(bits: u8) -> Vec<Kind> {
    KINDS.into_iter().enumerate().filter(|(at, _)| bits >> at & 1 == 1).map(|(_, kind)| kind).collect()
}
