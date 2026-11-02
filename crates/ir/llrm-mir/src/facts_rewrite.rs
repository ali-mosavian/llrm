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

    /// Whether the fact states what makes an instruction undefined where it
    /// should not run: what `dropUBImplyingAttrsAndMetadata` takes from a
    /// call that is moved to where it may not run (`noundef`, `nonnull`,
    /// `dereferenceable`, `align` and a range).
    pub fn implies_ub(self) -> bool {
        matches!(
            self,
            Fact::NoUndef | Fact::NonNull | Fact::Dereferenceable(_) | Fact::Align(_) | Fact::Range(_)
        )
    }

    /// Whether the fact still holds of an instruction run where it did not
    /// run. Every variant answers; see the module documentation.
    pub fn survives_speculation(self) -> bool {
        match self {
            Fact::NoAlias
            | Fact::ReadOnly
            | Fact::ReadNone
            | Fact::NonNull
            | Fact::NoUndef
            | Fact::NoCapture
            | Fact::NoRetain
            | Fact::Releases
            | Fact::WriteOnly
            | Fact::NoReturn
            | Fact::NoUnwind
            | Fact::NoFree
            | Fact::NoSync
            | Fact::WillReturn
            | Fact::NoCallback
            | Fact::Cold
            | Fact::ThreeWayCompare
            | Fact::FlagsResult
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
            | Fact::NoFarAllocation
            | Fact::MustProgress
            | Fact::NoRecurse
            | Fact::NoIpa
            | Fact::Access(_)
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

    /// `text` without its `#[cfg(test)]` items, where a test module may come
    /// before production code.
    fn production_source(text: &str) -> String {
        let mut kept = String::new();
        let mut depth = 0_i32;
        let mut skipping = false;
        let mut lines = text.lines().peekable();
        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if !skipping && depth == 0 && trimmed == "#[cfg(test)]" {
                skipping = true;
                continue;
            }
            if !skipping {
                kept.push_str(line);
                kept.push('\n');
                continue;
            }
            // Attributes between `#[cfg(test)]` and its item belong to it.
            if trimmed.starts_with("#[") {
                continue;
            }
            for (at, one) in line.char_indices() {
                match one {
                    '{' if !line[..at].contains("//") => depth += 1,
                    '}' if !line[..at].contains("//") => depth -= 1,
                    _ => {}
                }
            }
            // The item ends at its closing brace, or at a `;` outside braces.
            if depth <= 0 && (line.contains('}') || trimmed.ends_with(';')) {
                skipping = false;
                depth = 0;
            }
        }
        kept
    }

    /// The audit cut each file at its first `#[cfg(test)]`, so production code
    /// after an early test item was never read: `ranges.rs`'s `nsw` read hid
    /// behind one until the code before it was deleted.
    #[test]
    fn a_reader_after_an_early_test_item_is_audited() {
        let source = production_source(
            "#[cfg(test)]\nfn helper() {}\nfn real(f: &F) { f.no_signed_wrap(); }\n#[cfg(test)]\nmod tests {\n    fn t() { x.in_bounds(); }\n}\n",
        );
        assert!(source.contains("no_signed_wrap()"), "{source}");
        assert!(!source.contains("in_bounds()"), "{source}");
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
            // ValueTracking.cpp's `getRangeForInstruction` bounds an add,
            // sub, mul or shl by the `nsw` it has (`addWithNoWrap`,
            // `multiplyWithNoWrap`); the interval is worked out afresh from
            // the instruction when asked and kept by no pass.
            "ranges.rs",
            // CorrelatedValuePropagation.cpp's `processBinOp` and
            // `processTrunc` read the `nsw`/`nuw` the operation has and add
            // the ones LVI's ranges prove; neither drops one.
            "cvp.rs",
            // ConstantFolding.cpp reads a GEP's `inbounds`, `nusw` and `nuw`
            // to flag the constant that replaces it.
            "constantfolding.rs",
            // LowerMemIntrinsics.cpp makes the pattern loop's GEP `inbounds`;
            // `memsetpattern.rs` sets the flag on it and reads none.
            "memsetpattern.rs",
            // SelectionDAGBuilder.cpp copies `nsw`, `nuw` and a GEP's
            // `inbounds` into the node's SDNodeFlags (`visitBinary`,
            // `visitGetElementPtr`); the DAG grants no promise from them.
            "builder.rs",
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
            // SimplifyIndVar.cpp's `strengthenOverflowingOperation` gives an
            // add, sub or mul the `nuw`/`nsw` SCEV proves for it; it never
            // drops one.
            "simplifyindvar.rs",
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
            // ConstraintElimination.cpp's `getMonotonicityInfo` reads an
            // add's `nuw`/`nsw` and a GEP's `nuw`/`nusw` to say how an
            // induction steps; it never drops one.
            "constraintelim_induction.rs",
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
            // The same function's later folds: `-A + B` keeps `nsw` only if
            // both had it, `zext(B -nuw A)` needs `nuw`, and `(or A, B) +
            // (and A, B)` rewrites the operands in place to keep the flags.
            "combine_add_idioms.rs",
            // InstCombineNegator.cpp's `sub`/`shl`/`mul` cases give the
            // negated operation `nsw` only if the original and the negation
            // had it.
            "combine_negator.rs",
            // InstCombineAddSub.cpp's `visitSub` keeps `nsw`/`nuw` on the
            // sub it builds only where the operands and the original had
            // them, and infers them from the overflow queries.
            "combine_sub.rs",
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
            // InstCombineMulDivRem.cpp's `visitMul` and its helpers read the
            // `nuw`/`nsw` of a multiply and of the add, sub and shift it looks
            // through (`hasNoUnsignedWrap`, `hasNoSignedWrap`, `foldMulOp...`)
            // to give the replacement the flags the operands prove, and drop
            // the rest.
            "combine_muldivrem.rs",
            // InstCombineCasts.cpp's `visitTrunc`, `visitZExt` and `visitSExt`
            // read a trunc's, a zext's and a sext's `nuw`/`nsw`/`nneg`
            // (`hasNoUnsignedWrap`, `hasNoSignedWrap`, `hasNonNeg`) and give
            // the replacement the flags the narrower or wider operation
            // proves; `narrowBinOp` keeps an add's or sub's flags only where
            // they still hold.
            "combine_casts.rs",
            // InstructionCombining.cpp's `tryFactorization` reads the `nsw`/
            // `nuw` of the instruction and of the two operands it factors
            // (`OverflowingBinaryOperator`) and gives the factored result
            // `nuw`/`nsw` only where all three had them, an `add` of `mul`s
            // `nsw` only for a constant factor that is not INT_MIN; it never
            // gives one an expansion, and drops the rest.
            "combine_distribute.rs",
            // Reassociate.cpp's `OverflowTracking` (`mergeFlags`, `applyFlags`)
            // merges the `nuw`/`nsw` of the nodes of a tree so the rewritten
            // tree keeps them only if all had them; `ConvertShiftToMul` and
            // `NegateValue` read a shift's and an add's, and clear or give
            // them where LLVM does.
            "reassociate_forms.rs",
            "reassociate_negate.rs",
            "reassociate_tree.rs",
            // MemCpyOptimizer.cpp's `CreateInBoundsPtrAdd` makes an inbounds
            // GEP; it reads no flag.
            "memcpyopt.rs",
            // SimplifyCFG.cpp's `passingValueIsAlwaysUndefined` reads the
            // `inbounds` of the `getelementptr` that is the first witnessing
            // use of a phi, in the phi's block with nothing in between that may
            // not come back, so it runs wherever the phi's value does; it
            // concludes only about the value that flows in.
            "alwaysundef.rs",
            // InstCombineSimplifyDemanded.cpp reads the `nuw`/`nsw` of the
            // instruction it simplifies (`SimplifyDemandedUseBits`, the
            // known bits of an add, a sub and a shift) to conclude about that
            // instruction's result, and drops them
            // (`dropPoisonGeneratingFlags`) when an operand it changed no
            // longer licenses them;
            // `simplifyShrShlDemandedBits` gives the new shift the flags its
            // `shl` had.
            "combine_demanded.rs",
            // SimplifyCFG.cpp's `ConstantComparesGatherer::matchInstruction`
            // reads a `trunc nuw` (`m_NUWTrunc`, L646) of the chain it turns
            // into a switch: the trunc is an operand of the branch condition,
            // so it ran wherever the branch does, and a value the flag makes
            // poison made the branch undefined before.
            "icmpchain.rs",
            // SimplifyCFG.cpp's `isValueEqualityComparison` (L810) takes the
            // operand of a `trunc nuw` that is the condition of a branch as the
            // value compared: the trunc is an operand of the branch, so it ran
            // wherever the branch does, and a value the flag makes poison made
            // the branch undefined before.
            "switchvalue.rs",
            // SimplifyCFG.cpp's `switchToLookupTable` gives the `sub` that
            // makes the table index `nsw` when the case range cannot wrap;
            // it reads no flag.
            "switchlookup.rs",
            // InstCombineCompares.cpp's `foldICmpAndConstConst` builds the
            // `shl nuw 1, B` of the `(or (lshr A, B), A) & 1` fold
            // (`HasNUW=true`); it reads no flag.
            "combine_icmp_const.rs",
            // LICM.cpp's `hoistGEP` makes the swapped GEPs `inbounds` when both
            // were and no index is negative (L2373); `apply` gives the GEP it
            // builds the flag `hoist` decided.
            "apply.rs",
            // LoopAccessAnalysis.cpp's `isNoWrap` reads the `nusw` or
            // `inbounds` of the GEP an access goes through
            // (`hasNoUnsignedSignedWrap`) to call its add recurrence
            // non-wrapping, and only when an access through it runs in every
            // iteration (the GEP would be poison, the access UB, otherwise);
            // it changes no instruction.
            "loopaccess.rs",
            // LoopInterchange.cpp's `checkReductionKind` lists the reduction
            // operations that carry `nsw`/`nuw` (and `ninf`), and the
            // transform clears them, since a reordered sum may overflow; it
            // keeps no instruction's promise and grants none.
            "loopinterchange.rs",
            "loopinterchange_transform.rs",
            // CodeGenPrepare.cpp's `canGetThrough` reads the `nsw`/`nuw` of
            // the operand a sign or zero extension is moved up through: the
            // flag says the narrow operation did not wrap, so the same
            // operation on the extended operands computes the extension. The
            // promoted instruction keeps its flags (the pin's `mutateType`
            // does not touch them) and none is granted.
            "extpromotion.rs",
            // CodeGenPrepare.cpp's `isRemOfLoopIncrementWithLoopInvariant`
            // asks for `m_c_NUWAdd`: the counter's `add nuw` says it does not
            // wrap, which the wrapping remainder counter relies on; the
            // `nsw` of an add feeding the remainder is copied to the sum
            // simplified for the first remainder. The pass grants none.
            "uremloop.rs",
            // InstCombineSelect.cpp's `canonicalizeSPF` reads the `nsw` of the
            // negation arm (`m_NSWNeg`) of an absolute value to make `abs`
            // poison at the minimum: the negation was already poison there
            // on the arm the select takes. It grants no flag to an
            // instruction that had none.
            "combine_select_spf.rs",
            // LoopIdiomRecognize.cpp's `insertLoadTableForCRC` emits an
            // `inbounds` GEP into the CRC table it just created, indexed by
            // a byte. It reads no flag off an existing instruction.
            "crc.rs",
            // The `nsw` of a counter's `add phi, 1` bounds the counter to its
            // start and the loop's end, for the range of a fixed-point
            // product; the interval is worked out afresh when asked and kept
            // by no pass. It grants no flag.
            "fixednarrow.rs",
            // InstCombineCompares.cpp's `foldICmpBinOp` reads the `nuw`/`nsw`
            // of the two operands of a compare (`NoOp0WrapProblem`) to cancel
            // a common operand, and for `(X + C1) cmp (Y + C2)` gives the
            // single add it builds the flag the operand it came from had (
            // `Op0HasNUW`/`Op0HasNSW`, L5552): a flag no instruction in the
            // compare's operands lacked, on a value only the compare reads.
            "combine_icmp_binop.rs",
            // InstructionCombining.cpp's `EmitGEPOffsets` adds the offsets with
            // `CreateAdd(Sum, Offset, "", NW.hasNoUnsignedWrap(),
            // NW.isInBounds())`: the flags of the GEP chain's common no-wrap
            // flags go on the new add, which only the compare reads. It
            // reads none off an existing add.
            "combine_gep_base.rs",
            // InstructionCombining.cpp's `narrowMathIfNoOverflow` sets `nsw`
            // (sext) or `nuw` (zext) on the narrow operation it builds, once
            // `willNotOverflow` has shown the math cannot wrap at that width.
            // It reads no flag off an existing instruction.
            "combine_narrow.rs",
            // InstCombineAndOrXor.cpp's `foldCastedBitwiseLogic` reads the
            // `nuw`/`nsw` of the two truncs it merges: `trunc nuw` of an and
            // needs one operand `nuw`, of an or both, and `nsw` needs both.
            // The flags go on the new trunc only, which the logic result
            // proves; the old truncs keep theirs.
            "combine_casted_logic.rs",
            // The memcpy loop builds its two `gep inbounds i8, p, 1` from the
            // copy's own pointers; it reads no flag off an existing
            // instruction.
            "memloops.rs",
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
                    let source = production_source(&text);
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
