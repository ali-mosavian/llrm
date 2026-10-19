//! What a pass that rewrites code does to the facts on it: when it folds two
//! instructions into one, and when it runs one where it did not run.
//!
//! One `match` per question over every `Fact`, with no catch-all: a fact
//! added without an answer does not compile.
//!
//! **Merge** (GVN, EarlyCSE: the one instruction runs wherever either did).
//! A promise survives only if both made it, at the weaker of the two values.
//! Today no pass merges across a promise: flags are in every value-numbering
//! key, so `add nsw` and a plain `add` stay two instructions, and no pass
//! copies metadata between instructions. Whoever lets one do either, calls
//! this.
//!
//! **Speculate** (LICM, hoist, PRE insertion: the instruction runs where it
//! did not). A fact stated of an instruction is a poison-generating promise,
//! and LLVM's poison is lazy: it matters only where a user reads it, and the
//! users of a moved instruction are the ones its original had, which only ran
//! after the original did. So every fact here survives. A fact that made the
//! instruction undefined behaviour outright (LLVM's `noundef`,
//! `!dereferenceable` on a load) would not, and answers `false` below. Checked
//! when this was written, on the passes `llrm_transforms::pipeline` runs:
//! `hoist`, `loopmotion`, `lsr` (induction steps), `exitsink` (through
//! `indvars`), `gvn::joined` (insertion on an edge) and GVN's load PRE (a load
//! made in a predecessor) move or insert an instruction with its flags, and
//! are sound by the argument above; `algebraic` reassociation changes operands
//! and already clears the flags (`set_flags(.., Flags::default())`). The
//! optimiser passes `llrm-mir` once had were never run by the compile route
//! (#237) and are gone; a pass added to the route comes under this audit.
//!
//! The argument needs one more thing: no reader takes a flag or `!range` off
//! an instruction to conclude something about its operands, or about another
//! point, without the instruction executing wherever it concludes. LLVM drops
//! flags when it hoists for exactly that. Checked: `induction::_promised`
//! reads a counter's step, which flows into the header phi along the back
//! edge and so ran on every trip it matters to (and depends on the phi, so is
//! never hoisted); `induction::inbounds_backedges` requires the access to
//! dominate the latch; `memory` and `pointerfacts` conclude only that a GEP's
//! result is in its base's object; `gepoffset` rewrites `sext (add nsw ..)` at
//! the add; `isel` prices a wrapping add; `algebraic::_offset_compared` reads
//! the flag of a sum to fold a compare of it (`icmp P (x + c1), c2` is `icmp P
//! x, c2 - c1` where the sum cannot wrap): the compare is a user of the sum, so
//! the sum ran wherever the compare does, the new compare is made in the old
//! one's place, and where the sum would have wrapped it is poison and either
//! answer stands. `simplify` reads the flags of the instructions that define
//! the operands of the instruction it simplifies, as LLVM's
//! `InstructionSimplify` does (`(X <<nuw Y) >> Y` is `X`): each is an operand,
//! so it ran wherever the user does, and the answer replaces that user only.
//! `constantfolding` reads the flags of the GEP it folds to flag the constant
//! that replaces that GEP, in its place and for its users only.
//! `ranges`, `guards` and `consts` read no flag. Value tracking
//! (`valuetracking/`: `context`, `guarantee`, `known`, `nonzero`, `power2`,
//! `range`, and the matchers in `pattern`) reads a promise of the instruction
//! it is asked about, or of an operand of it, to conclude about that
//! instruction's result, as LLVM does: the result is poison where the promise
//! fails, and poison matters only where it is read. It reads one to conclude
//! about an operand, or about another point, in four places, each with its
//! proof that the instruction runs there. A comparison that an assume or a
//! dominating branch makes true gives bits and ranges of its operand from the
//! `nuw`/`nsw` of the operation it compares, `trunc nuw` and `samesign`
//! (`computeKnownBitsFromCmp` L886, `computeKnownBitsFromICmpCond` L992,
//! `computeKnownBitsFromCond` L1014, `computeKnownBitsFromContext` L1057,
//! `computeConstantRange` L10755): the assume is valid for the context
//! (`isValidAssumeForContext`) or the edge dominates it. A recurrence reads the
//! flags of the binary operator that is the phi's back-edge value (L1847,
//! L2696, L3162, L4032): it ran on every trip, and it uses the phi, so it is
//! never hoisted above it. `isKnownNonNullFromDominatingCondition` (L3048)
//! reads the `nonnull` of a call's argument at a call that dominates the
//! context. `handleGuaranteedWellDefinedOps` (L8556) reads `noundef` and
//! `dereferenceable` arguments of a call that `programUndefinedIfUndefOrPoison`
//! reaches from the value through instructions that transfer execution. The
//! test below lists the files that name a no-wrap or inbounds flag, so a
//! new reader is checked before it is added.

use crate::facts::{Bounds, Effect, Fact, Facts};

impl Fact {
    /// The fact that holds of the one instruction two stated instructions
    /// are folded into, where `self` was stated of one and `other` of the
    /// other. `None` where nothing is promised of both.
    pub fn merged(
        self,
        other: Fact,
    ) -> Option<Fact> {
        match (self, other) {
            (Fact::Dereferenceable(a), Fact::Dereferenceable(b)) => Some(Fact::Dereferenceable(a.min(b))),
            (Fact::Align(a), Fact::Align(b)) => Some(Fact::Align(a.min(b))),
            (Fact::Initializes(a), Fact::Initializes(b)) => Some(Fact::Initializes(a.min(b))),
            (Fact::Memory(a), Fact::Memory(b)) => Self::effects(a, b).map(Fact::Memory),
            // Both bounds hold of the one value that is either's: the hull.
            (Fact::Range(a), Fact::Range(b)) => Some(Fact::Range(Bounds { lo: a.lo.min(b.lo), hi: a.hi.max(b.hi) })),
            (Fact::Unroll(a), Fact::Unroll(b)) => Some(Fact::Unroll(a.min(b))),
            (a, b) if a == b => Some(a),
            _ => None,
        }
    }

    /// The weaker of two memory effects: what either may do, if that is one of
    /// them.
    fn effects(
        a: Effect,
        b: Effect,
    ) -> Option<Effect> {
        match (a, b) {
            (a, b) if a == b => Some(a),
            (Effect::None, other) | (other, Effect::None) => Some(other),
            _ => None,
        }
    }

    /// Whether the fact still holds of an instruction run where it did not
    /// run. Every variant answers; see the module documentation.
    pub fn survives_speculation(self) -> bool {
        match self {
            Fact::NoAlias
            | Fact::ReadOnly
            | Fact::ReadNone
            | Fact::NonNull
            | Fact::NoCapture
            | Fact::NoRetain
            | Fact::Releases
            | Fact::WriteOnly
            | Fact::NoReturn
            | Fact::NoUnwind
            | Fact::WillReturn
            | Fact::NoCallback
            | Fact::Cold
            | Fact::ThreeWayCompare
            | Fact::StackCheck
            | Fact::Dereferenceable(_)
            | Fact::Align(_)
            | Fact::Initializes(_)
            | Fact::Memory(_)
            | Fact::NoSignedWrap
            | Fact::NoUnsignedWrap
            | Fact::InBounds
            | Fact::Inline(_)
            | Fact::Reassoc
            | Fact::NoNaNs
            | Fact::NoInfs
            | Fact::NoSignedZeros
            | Fact::AllowReciprocal
            | Fact::Invariant
            | Fact::Unroll(_)
            | Fact::MustProgress
            | Fact::NoRecurse
            | Fact::NoIpa
            | Fact::Range(_) => true,
        }
    }
}

impl Facts {
    /// What two stated sets both promise, at the weaker value of each.
    pub fn merged(
        &self,
        other: &Facts,
    ) -> Facts {
        let mut kept = Vec::new();
        for fact in self.iter() {
            let shared = other.iter().filter_map(|one| fact.merged(one)).next();
            kept.extend(shared.filter(|one| !kept.contains(one)));
        }
        Facts::from_facts(kept)
    }

    /// What still holds of an instruction run where it did not run.
    pub fn speculated(&self) -> Facts {
        Facts::from_facts(self.iter().filter(|fact| fact.survives_speculation()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(list: &[Fact]) -> Facts {
        Facts::from_facts(list.to_vec())
    }

    /// Two instructions folded keep what both promised, at the weaker value:
    /// `add nsw` with a plain `add` is a plain `add`.
    #[test]
    fn a_merge_keeps_only_what_both_promised_at_the_weaker_value() {
        let both =
            facts(&[Fact::NoSignedWrap, Fact::NoUnsignedWrap, Fact::Dereferenceable(8), Fact::Memory(Effect::None)]);
        let other = facts(&[Fact::NoSignedWrap, Fact::Dereferenceable(4), Fact::Memory(Effect::Read)]);
        let merged = both.merged(&other);
        assert!(merged.no_signed_wrap() && !merged.no_unsigned_wrap());
        assert_eq!(merged.dereferenceable(), Some(4));
        assert_eq!(merged.memory(), Some(Effect::Read));
        assert_eq!(
            facts(&[Fact::NoSignedWrap]).merged(&facts(&[])).iter().count(),
            0,
            "one side's promise alone is not kept"
        );
        assert_eq!(facts(&[Fact::Memory(Effect::Read)]).merged(&facts(&[Fact::Memory(Effect::Write)])).memory(), None);
        let hull =
            facts(&[Fact::Range(Bounds { lo: 0, hi: 3 })]).merged(&facts(&[Fact::Range(Bounds { lo: 2, hi: 9 })]));
        assert_eq!(hull.range(), Some(Bounds { lo: 0, hi: 9 }), "a merged range is the hull");
    }

    /// Every fact survives being run where it did not run, today: the users
    /// of the moved instruction are the original's.
    #[test]
    fn speculation_keeps_every_fact_today() {
        let all = Facts::from_facts(Fact::examples());
        assert_eq!(all.speculated().iter().count(), all.iter().count());
    }

    /// Every file that reads a no-wrap or inbounds promise off an
    /// instruction is one whose conclusion was checked (see the module
    /// documentation); a new one fails here until it is, and is listed.
    #[test]
    fn only_audited_readers_take_a_promise_off_an_instruction() {
        const CHECKED: &[&str] = &[
            "algebraic.rs",
            "simplify.rs",
            "induction.rs",
            "memory.rs",
            "gepoffset.rs",
            "isel.rs",
            "interpret.rs",
            "mir.rs",
            "pointerfacts.rs",
            "context.rs",
            "guarantee.rs",
            "known.rs",
            "nonzero.rs",
            "pattern.rs",
            "power2.rs",
            "range.rs",
            // ConstantFolding.cpp reads a GEP's `inbounds`, `nusw` and `nuw`
            // to flag the constant that replaces it.
            "constantfolding.rs",
            // InlineCost.cpp reads a GEP's `inbounds` where LLVM does
            // (`stripAndAccumulateInBoundsConstantOffsets`,
            // `visitGetElementPtr`).
            "inlinecost.rs",
            // ScalarEvolution.cpp reads them in `createSCEV`
            // and `MatchBinaryOp`
            // (`getNoWrapFlagsFromUB`, `getNoWrapFlagsForGEP`); a pass that
            // drops one forgets the value, as LLVM's do (`forgetValue`).
            "create.rs",
            "create_phi.rs",
            "create_addrec.rs",
            // ScalarEvolutionExpander.cpp's `InsertBinop` reuses a nearby
            // binop only if its `nsw`/`nuw` equal the requested ones
            // (`canGenerateIncompatiblePoison`).
            "expand.rs",
            // `getAddRecExprPHILiterally` sets `nuw`/`nsw` on the increment
            // `IsIncrementNUW`/`NSW` prove.
            "expand_iv.rs",
            // Local.cpp's `andIRFlags` and `dropPoisonGeneratingFlags`, and
            // GVN.cpp's `m_NUWTrunc` in `propagateEquality`, read them where
            // LLVM does.
            "local.rs",
            "pass.rs",
            // SimplifyIndVar.cpp's `WidenIV` reads an add's/sub's/mul's
            // `nsw`/`nuw` (`matchBinaryOp`, `getExtendedOperandRecurrence`,
            // `widenWithVariantUse`) to extend its operands, and `copyIRFlags`
            // gives the wide clone the narrow flags it proved.
            "widen.rs",
            // ScalarEvolution.cpp's `getStrengthenedNoWrapFlagsFromBinOp`
            // reads the flags the operation has and adds the ones the
            // no-wrap proofs give (`willNotOverflow`); it never drops one.
            "how_many.rs",
            // IndVarSimplify.cpp's `linearFunctionTestReplace` clears the
            // increment's `nuw`/`nsw` that SCEV did not prove for its
            // post-inc add recurrence; it never adds one.
            "lftr.rs",
            // PHITransAddr.cpp's `translateSubExpr` reads an add's
            // `nsw`/`nuw` for `simplifyAddInst`, and `insertTranslatedSubExpr`
            // copies a GEP's no-wrap flags to the GEP it makes.
            "phitrans.rs",
            // LazyValueInfo.cpp's `binaryOp(BO)`, `Trunc->getNoWrapKind()`,
            // `m_NUWMul` and `m_AddLike`, and `stripInBoundsOffsets`, read
            // them where LLVM does.
            "lazyvalueinfo.rs",
            // ConstraintElimination.cpp reads nuw/inbounds/nsw where LLVM
            // does (`decompose`, `getFlagsFor...`).
            "constraintelim.rs",
            // BasicAliasAnalysis.cpp's `DecomposeGEPExpression` reads a GEP's
            // `inbounds` and `nuw` where LLVM does; `aliasGEP` takes NoAlias
            // from them only as LLVM's rules do.
            "basicaa.rs",
            // SROA.cpp's use of `Value::stripInBoundsOffsets`: it walks a
            // GEP only when it is `inbounds`, where LLVM does.
            "sroa.rs",
            // LoopUnroll.cpp's `simplifyLoopAfterUnroll` folds
            // `(x + c1) + c2` and keeps `nuw`/`nsw` only if both adds had
            // them, and `c1 + c2` did not overflow.
            "loopunrollpass.rs",
            // DemandedBits.cpp's `determineLiveOperandBits` reads an
            // add's/sub's/mul's `nsw`/`nuw` to see one more high bit live;
            // it never drops one.
            "demandedbits.rs",
            // SCCPSolver.cpp's `refineInstruction` reads the wrap flags it is
            // about to add (`nuw`, `nsw`, `nneg`, a GEP's `nuw`) and never
            // drops one.
            "sccp.rs",
            // `visitGetElementPtrInst` reads a GEP's `nuw`/`inbounds` for
            // not-null, and `visitCastInst` a `trunc`'s no-wrap kind for the
            // result range, where LLVM does.
            "sccpsolver.rs",
            // InstCombineCompares.cpp's `foldICmpWithZero` reads a multiply's
            // `nuw`/`nsw` (`BO0->hasNoUnsignedWrap()`) to drop it from an
            // equality with zero; it never drops one.
            "combine_compares.rs",
            // PredicateInfo.cpp's `m_NUWTrunc(m_Value(Op0))` in
            // `processBranch`, `processAssume` and `getConstraint`.
            "predicateinfo.rs",
            // InstructionCombining.cpp's `SimplifyAssociativeOrCommutative`
            // reads `nuw`/`nsw` (`hasNoUnsignedWrap`, `maintainNoSignedWrap`)
            // to set them on the reassociated operation only when both
            // operations had them, and drops the rest.
            "combine_shared.rs",
            // InstCombineAddSub.cpp's `visitAdd` and its helpers read an add's
            // and a sub's `nsw`/`nuw` where LLVM does, to give the
            // replacement the flags the operands prove.
            "combine_addsub.rs",
            // InstructionCombining.cpp's `foldBinopWithRecurrence` merges the
            // `nuw`/`nsw` of the three adds (`OverflowTracking`) so the new add
            // has them only if all had them, and drops the rest.
            "combine_phi.rs",
            // InstructionCombining.cpp's `visitGetElementPtrInst` and
            // `emitGEPOffset` read a GEP's `inbounds`/`nusw`/`nuw` to give the
            // replacement GEPs and the offset arithmetic the flags the GEP had.
            "combine_gep.rs",
            "combine_gep_index.rs",
            "combine_gep_merge.rs",
            "combine_gep_phi.rs",
            // InstCombineShifts.cpp reads a shift's `nuw`/`nsw`/`exact` and the
            // flags of the add, sub and mul it looks through (`visitShl`,
            // `visitLShr`, `visitAShr`, `setShiftFlags`) to give the
            // replacement the flags the operands prove; `getShiftedValue`
            // drops an add's flags when it shifts the add left.
            "combine_shifts.rs",
            // InstCombineCasts.cpp's `visitTrunc`, `visitZExt` and `visitSExt`
            // read a trunc's, a zext's and a sext's `nuw`/`nsw`/`nneg`
            // (`hasNoUnsignedWrap`, `hasNoSignedWrap`, `hasNonNeg`) and give
            // the replacement the flags the narrower or wider operation
            // proves; `narrowBinOp` keeps an add's or sub's flags only where
            // they still hold.
            "combine_casts.rs",
            // MemCpyOptimizer.cpp's `CreateInBoundsPtrAdd` makes an inbounds
            // GEP; it reads no flag.
            "memcpyopt.rs",
        ];
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let reads =
            ["no_signed_wrap()", "no_unsigned_wrap()", "in_bounds()", "Flags::NSW", "Flags::NUW", "Flags::INBOUNDS"];
        let mut found = Vec::new();
        let mut directories = vec![root];
        while let Some(directory) = directories.pop() {
            for entry in std::fs::read_dir(&directory).expect("a directory").flatten() {
                let path = entry.path();
                let name = path.file_name().and_then(|one| one.to_str()).unwrap_or_default().to_owned();
                if path.is_dir() {
                    if !matches!(name.as_str(), "target" | "tests" | "fixtures") {
                        directories.push(path);
                    }
                } else if name.ends_with(".rs")
                    && !name.contains("test")
                    && !CHECKED.contains(&name.as_str())
                    && !matches!(
                        name.as_str(),
                        "facts.rs" | "facts_rewrite.rs" | "parse.rs" | "print.rs" | "opcode.rs" | "edit.rs"
                    )
                {
                    let text = std::fs::read_to_string(&path).expect("source");
                    let source = text.split("#[cfg(test)]").next().unwrap_or_default();
                    if reads.iter().any(|read| source.contains(read)) {
                        found.push(name);
                    }
                }
            }
        }
        found.sort();
        assert_eq!(found, Vec::<String>::new(), "a reader of nsw/nuw/inbounds that was not audited");
    }
}
