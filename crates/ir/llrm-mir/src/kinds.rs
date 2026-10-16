//! What kind of instruction an edit reached: one vocabulary for what a pass
//! looks at (`FunctionPass::reads`) and what an analysis models
//! (`Depends`), asked of the one digest (`Touched`).
//!
//! A [`Kinds`] is a set of instruction classes. A load, a store, a phi and a
//! select are told by the class of the value they carry (integer, float,
//! pointer, aggregate), so a pass or an analysis that models pointer stores
//! only is not reached by an integer one. [`Reads`] says what a reader looks
//! at (`kinds`), which erased instructions it holds entries for (`keyed`), and
//! where a change flows (`stops`: the classes past which a value is not
//! followed to its users). [`Touched::leaves`](crate::touched::Touched::leaves)
//! asks whether a change set leaves a reader's result as it was, in LLVM's
//! words whether a pass `preserved` it; `meets` and `blocks_meeting` say which
//! readers an edit reached and where.

use crate::context::Context;
use crate::module::{Function, InstId};
use crate::opcode::{CastOp, Opcode};
use crate::types::Type;

/// A set of instruction classes and of the non-instruction changes a reader
/// can depend on.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Kinds(u32);

impl Kinds {
    pub const NONE: Self = Self(0);

    // Typed classes: four bits each, integer, float, pointer, aggregate.
    pub const LOAD_INT: Self = Self(1);
    pub const LOAD_FLOAT: Self = Self(1 << 1);
    pub const LOAD_PTR: Self = Self(1 << 2);
    pub const LOAD_AGG: Self = Self(1 << 3);
    pub const STORE_INT: Self = Self(1 << 4);
    pub const STORE_FLOAT: Self = Self(1 << 5);
    pub const STORE_PTR: Self = Self(1 << 6);
    pub const STORE_AGG: Self = Self(1 << 7);
    pub const PHI_INT: Self = Self(1 << 8);
    pub const PHI_FLOAT: Self = Self(1 << 9);
    pub const PHI_PTR: Self = Self(1 << 10);
    pub const PHI_AGG: Self = Self(1 << 11);
    pub const SELECT_INT: Self = Self(1 << 12);
    pub const SELECT_FLOAT: Self = Self(1 << 13);
    pub const SELECT_PTR: Self = Self(1 << 14);
    pub const SELECT_AGG: Self = Self(1 << 15);
    // Classes of one kind of value.
    pub const CALL: Self = Self(1 << 16);
    pub const INVOKE: Self = Self(1 << 17);
    pub const ALLOCA: Self = Self(1 << 18);
    pub const GEP: Self = Self(1 << 19);
    /// A cast between a pointer and an integer, or of a pointer (`ptrtoint`,
    /// `inttoptr`, `bitcast`, `addrspacecast`).
    pub const CAST_PTR: Self = Self(1 << 20);
    pub const CAST: Self = Self(1 << 21);
    /// Arithmetic, a comparison, a negation.
    pub const ARITH: Self = Self(1 << 22);
    /// `extractvalue`, `insertvalue`.
    pub const AGGREGATE: Self = Self(1 << 23);
    pub const FREEZE: Self = Self(1 << 24);
    /// `br`, `switch`: a terminator that decides control.
    pub const BRANCH: Self = Self(1 << 25);
    /// `ret`.
    pub const RET: Self = Self(1 << 26);
    /// `unreachable`, `resume`.
    pub const EXIT: Self = Self(1 << 27);
    pub const LANDINGPAD: Self = Self(1 << 28);
    /// A block made or erased, or a CFG edge changed.
    pub const CFG: Self = Self(1 << 29);
    /// The parameters or the attributes of the function.
    pub const SIGNATURE: Self = Self(1 << 30);

    pub const LOAD: Self = Self(0b1111);
    pub const STORE: Self = Self(0b1111 << 4);
    pub const PHI: Self = Self(0b1111 << 8);
    pub const SELECT: Self = Self(0b1111 << 12);
    /// The classes of integers, of floats, of pointers, of aggregates, across
    /// the typed ones.
    pub const INT: Self = Self(Self::LOAD_INT.0 | Self::STORE_INT.0 | Self::PHI_INT.0 | Self::SELECT_INT.0);
    pub const FLOAT: Self = Self(Self::LOAD_FLOAT.0 | Self::STORE_FLOAT.0 | Self::PHI_FLOAT.0 | Self::SELECT_FLOAT.0);
    pub const PTR: Self = Self(Self::LOAD_PTR.0 | Self::STORE_PTR.0 | Self::PHI_PTR.0 | Self::SELECT_PTR.0);
    pub const AGG: Self = Self(Self::LOAD_AGG.0 | Self::STORE_AGG.0 | Self::PHI_AGG.0 | Self::SELECT_AGG.0);
    /// Every terminator.
    pub const TERMINATOR: Self = Self(Self::BRANCH.0 | Self::RET.0 | Self::EXIT.0 | Self::INVOKE.0);
    /// What does something besides make its value (`depends::effectful`): a
    /// store, a call, a landing pad, a terminator. A volatile load is told
    /// from the others by the reader, not here.
    pub const EFFECTFUL: Self = Self(Self::STORE.0 | Self::CALL.0 | Self::LANDINGPAD.0 | Self::TERMINATOR.0);
    pub const ALL: Self = Self((1 << 31) - 1);

    pub fn meets(
        self,
        other: Self,
    ) -> bool {
        self.0 & other.0 != 0
    }

    pub fn contains(
        self,
        other: Self,
    ) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn with(
        self,
        other: Self,
    ) -> Self {
        Self(self.0 | other.0)
    }

    pub fn without(
        self,
        other: Self,
    ) -> Self {
        Self(self.0 & !other.0)
    }

    pub fn is_none(self) -> bool {
        self.0 == 0
    }

    /// The names of the classes in the set, for the reason a reader was
    /// reached (`LLRM_WHY`).
    pub fn names(self) -> Vec<&'static str> {
        const NAMES: &[(Kinds, &str)] = &[
            (Kinds::LOAD_INT, "load int"),
            (Kinds::LOAD_FLOAT, "load float"),
            (Kinds::LOAD_PTR, "load ptr"),
            (Kinds::LOAD_AGG, "load agg"),
            (Kinds::STORE_INT, "store int"),
            (Kinds::STORE_FLOAT, "store float"),
            (Kinds::STORE_PTR, "store ptr"),
            (Kinds::STORE_AGG, "store agg"),
            (Kinds::PHI_INT, "phi int"),
            (Kinds::PHI_FLOAT, "phi float"),
            (Kinds::PHI_PTR, "phi ptr"),
            (Kinds::PHI_AGG, "phi agg"),
            (Kinds::SELECT_INT, "select int"),
            (Kinds::SELECT_FLOAT, "select float"),
            (Kinds::SELECT_PTR, "select ptr"),
            (Kinds::SELECT_AGG, "select agg"),
            (Kinds::CALL, "call"),
            (Kinds::INVOKE, "invoke"),
            (Kinds::ALLOCA, "alloca"),
            (Kinds::GEP, "gep"),
            (Kinds::CAST_PTR, "cast ptr"),
            (Kinds::CAST, "cast"),
            (Kinds::ARITH, "arith"),
            (Kinds::AGGREGATE, "aggregate"),
            (Kinds::FREEZE, "freeze"),
            (Kinds::BRANCH, "branch"),
            (Kinds::RET, "ret"),
            (Kinds::EXIT, "exit"),
            (Kinds::LANDINGPAD, "landingpad"),
            (Kinds::CFG, "cfg"),
            (Kinds::SIGNATURE, "signature"),
        ];
        NAMES.iter().filter(|(kind, _)| self.meets(*kind)).map(|(_, name)| *name).collect()
    }
}

impl std::ops::BitOr for Kinds {
    type Output = Self;

    fn bitor(
        self,
        other: Self,
    ) -> Self {
        self.with(other)
    }
}

/// What a reader (a pass, an analysis) looks at of a function.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Reads {
    // `kinds` holds `Kinds::CFG` where a block made or erased, or an edge
    // changed, is a change the reader sees, and `Kinds::SIGNATURE` where the
    // parameters are: an analysis, whose result has a part for every block and
    // every parameter, takes both (`Reads::analysis`).
    /// The classes of instruction whose change it can see.
    pub kinds: Kinds,
    /// The classes it holds an entry for, which erasing an instruction of the
    /// class removes.
    pub keyed: Kinds,
    /// A change to an instruction reaches its value's users, past any user of
    /// these classes (`None`: it does not flow, a change is the instruction's
    /// own).
    pub stops: Option<Kinds>,
}

/// The type class of `ty`: which of the integer, float, pointer and aggregate
/// bits a typed class takes (an integer's, for anything that is none: a
/// vector of integers is an aggregate here, a void has none).
fn type_class(
    context: &Context,
    ty: crate::types::TypeId,
) -> u32 {
    match context.types.get(ty) {
        Type::Int(_) => 0,
        Type::Float(_) => 1,
        Type::Pointer(_) => 2,
        Type::Void | Type::Label | Type::Metadata | Type::Token => 0,
        _ => 3,
    }
}

/// The class of one instruction, as it stands now (an erased instruction
/// keeps its opcode and operands in the arena).
pub fn of(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Kinds {
    let one = function.instruction(inst);
    let typed =
        |base: u32, ty: Option<crate::types::TypeId>| Kinds(1 << (base + ty.map_or(0, |ty| type_class(context, ty))));
    match &one.opcode {
        Opcode::Load { .. } => typed(0, Some(one.ty)),
        Opcode::Store { .. } => {
            typed(4, one.operands.first().and_then(|operand| function.operand_type(context, *operand)))
        }
        Opcode::Phi => typed(8, Some(one.ty)),
        Opcode::Select => typed(12, Some(one.ty)),
        Opcode::Call(_) => Kinds::CALL,
        Opcode::Invoke(_) => Kinds::INVOKE,
        Opcode::Alloca { .. } => Kinds::ALLOCA,
        Opcode::GetElementPtr { .. } => Kinds::GEP,
        Opcode::Cast(CastOp::PtrToInt | CastOp::IntToPtr | CastOp::BitCast | CastOp::AddrSpaceCast) => Kinds::CAST_PTR,
        Opcode::Cast(_) => Kinds::CAST,
        Opcode::Binary(_) | Opcode::FNeg | Opcode::ICmp(_) | Opcode::FCmp(_) => Kinds::ARITH,
        Opcode::ExtractValue(_) | Opcode::InsertValue(_) => Kinds::AGGREGATE,
        Opcode::Freeze => Kinds::FREEZE,
        Opcode::Br | Opcode::Switch => Kinds::BRANCH,
        Opcode::Ret => Kinds::RET,
        Opcode::Unreachable | Opcode::Resume => Kinds::EXIT,
        Opcode::LandingPad { .. } => Kinds::LANDINGPAD,
    }
}

impl Reads {
    /// A reader that sees every change of the CFG and of the signature, as
    /// `Depends` does: its result has a part for each block and each parameter.
    pub fn analysis(
        kinds: Kinds,
        keyed: Kinds,
        stops: Option<Kinds>,
    ) -> Self {
        Self { kinds: kinds | Kinds::CFG | Kinds::SIGNATURE, keyed, stops }
    }
}

/// What one window of edits touched, by class and by block, found once from its
/// `Touched` for the readers that ask again and again (a pass per step of a
/// round): `meets` and `blocks_meeting` cost the classes, not the edits.
/// Windows add (`merge`), so what was edited since some step ran, apart from a
/// pass's own edits, is the sum of the other steps' windows, not the log read
/// again.
///
/// It answers the two flows the passes declare: none (`Reads::stops` is
/// `None`, a change is the instruction's own) and unbounded (`Some(NONE)`,
/// followed to every user); a reader that stops the flow at some classes asks
/// the `Touched` (`Touched::leaves`).
#[derive(Clone, Debug, Default)]
pub struct Edits {
    /// The classes of the instructions added, moved or rewritten.
    direct: Kinds,
    /// Those and the classes of every instruction their values reach.
    flowed: Kinds,
    /// The classes of the instructions erased; of those, the ones whose
    /// opcode does something besides make a value.
    erased: Kinds,
    erased_effectful: Kinds,
    /// A block was made or erased; the parameters changed.
    blocks_changed: bool,
    signature: bool,
    /// The blocks of the instructions of a class touched, and reached, by class
    /// bit; and every block an edit was in, for what an erased instruction
    /// leaves no block of.
    direct_blocks: Vec<(u32, crate::dense::IdSet<crate::module::BlockId>)>,
    flowed_blocks: Vec<(u32, crate::dense::IdSet<crate::module::BlockId>)>,
    touched_blocks: crate::dense::IdSet<crate::module::BlockId>,
}

fn add_block(
    by_class: &mut Vec<(u32, crate::dense::IdSet<crate::module::BlockId>)>,
    class: Kinds,
    block: crate::module::BlockId,
) {
    for bit in (0..31).map(|at| 1u32 << at).filter(|bit| class.0 & bit != 0) {
        match by_class.iter_mut().find(|(one, _)| *one == bit) {
            Some((_, blocks)) => {
                blocks.insert(block);
            }
            None => {
                let mut blocks = crate::dense::IdSet::new();
                blocks.insert(block);
                by_class.push((bit, blocks));
            }
        }
    }
}

impl Edits {
    pub fn of(
        touched: &crate::touched::Touched,
        context: &Context,
        function: &Function,
    ) -> Self {
        let mut found = Self {
            blocks_changed: !touched.blocks_created.is_empty() || !touched.blocks_erased.is_empty(),
            signature: touched.parameters,
            touched_blocks: touched.blocks.clone(),
            ..Self::default()
        };
        for inst in touched.erased.iter() {
            let class = of(context, function, inst);
            found.erased = found.erased | class;
            if crate::depends::effectful(&function.instruction(inst).opcode) {
                found.erased_effectful = found.erased_effectful | class;
            }
        }
        let mut work: Vec<InstId> = touched.insts.iter().collect();
        let mut seen = touched.insts.clone();
        for inst in touched.insts.iter() {
            let class = of(context, function, inst);
            found.direct = found.direct | class;
            if let Some(block) = function.parent(inst) {
                add_block(&mut found.direct_blocks, class, block);
            }
        }
        while let Some(inst) = work.pop() {
            let class = of(context, function, inst);
            found.flowed = found.flowed | class;
            if let Some(block) = function.parent(inst) {
                add_block(&mut found.flowed_blocks, class, block);
            }
            let Some(result) = function.instruction(inst).result else { continue };
            for user in function.users(result) {
                if seen.insert(user.user) {
                    work.push(user.user);
                }
            }
        }
        found
    }

    /// The edits of `other` as well.
    pub fn merge(
        &mut self,
        other: &Self,
    ) {
        self.direct = self.direct | other.direct;
        self.flowed = self.flowed | other.flowed;
        self.erased = self.erased | other.erased;
        self.erased_effectful = self.erased_effectful | other.erased_effectful;
        self.blocks_changed |= other.blocks_changed;
        self.signature |= other.signature;
        self.touched_blocks.extend(other.touched_blocks.iter());
        for (mine, theirs) in
            [(&mut self.direct_blocks, &other.direct_blocks), (&mut self.flowed_blocks, &other.flowed_blocks)]
        {
            for (bit, blocks) in theirs {
                match mine.iter_mut().find(|(one, _)| one == bit) {
                    Some((_, held)) => held.extend(blocks.iter()),
                    None => mine.push((*bit, blocks.clone())),
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.direct.is_none() && self.erased.is_none() && !self.blocks_changed && !self.signature
    }

    /// The classes an edit reached: touched, or the value of one that was.
    pub fn reached(
        &self,
        flowing: bool,
    ) -> Kinds {
        if flowing { self.flowed } else { self.direct }
    }

    /// Whether the edits reach a reader of `reads`; none for a flow this does
    /// not keep (a bounded `stops`).
    pub fn meets(
        &self,
        reads: &Reads,
    ) -> Option<bool> {
        let flowing = Self::flow(reads)?;
        Some(
            (self.blocks_changed && reads.kinds.meets(Kinds::CFG))
                || (self.signature && reads.kinds.meets(Kinds::SIGNATURE))
                || reads.keyed.meets(self.erased)
                || reads.kinds.meets(self.erased_effectful)
                || reads.kinds.meets(self.reached(flowing)),
        )
    }

    /// The blocks of the instructions the edits touched or reached that `reads`
    /// models; none where it cannot say (the CFG or the signature changed for a
    /// reader that sees them, or a flow this does not keep).
    pub fn blocks_meeting(
        &self,
        reads: &Reads,
    ) -> Option<crate::dense::IdSet<crate::module::BlockId>> {
        let flowing = Self::flow(reads)?;
        if (self.blocks_changed && reads.kinds.meets(Kinds::CFG))
            || (self.signature && reads.kinds.meets(Kinds::SIGNATURE))
        {
            return None;
        }
        let mut blocks = crate::dense::IdSet::new();
        // An erased instruction leaves no block: every block of an edit.
        if reads.keyed.meets(self.erased) || reads.kinds.meets(self.erased) {
            blocks.extend(self.touched_blocks.iter());
        }
        let by_class = if flowing { &self.flowed_blocks } else { &self.direct_blocks };
        for (bit, held) in by_class {
            if reads.kinds.0 & bit != 0 {
                blocks.extend(held.iter());
            }
        }
        Some(blocks)
    }

    fn flow(reads: &Reads) -> Option<bool> {
        match reads.stops {
            None => Some(false),
            Some(stops) if stops.is_none() => Some(true),
            Some(_) => None,
        }
    }
}
