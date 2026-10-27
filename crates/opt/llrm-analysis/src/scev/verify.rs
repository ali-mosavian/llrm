//! `ScalarEvolution::verify`: what a kept `ScalarEvolution` holds is what a
//! new one would work out.
//!
//! # How it works
//! Translated from
//! [ScalarEvolution::verify](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14821).
//! A `ScalarEvolution` that lives across passes is right only while every pass
//! tells it what it changed (`forgetValue`, `forgetLoop`, ...). `verify` asks
//! a new one for the expression of each value the kept one has, maps the kept
//! expression into the new one's universe (`SCEVMapper`) and subtracts: a
//! nonzero constant difference is a stale expression, any other is let pass,
//! as the pin does without `-verify-scev-strict` (a value that went from an
//! opaque unknown to a recurrence, or whose no-wrap flags a later query
//! proved, is legal). `ValueExprMap` and `ExprValueMap` must name each other.
//!
//! DEVIATE V1 (no number): the cached trip counts, the user maps
//! (`SCEVUsers`, `ValuesAtScopes`, `BECountUsers`), the disposition caches,
//! the fold cache and `ConstantMultipleCache` are not checked. Exit: each
//! translated, as the persistent `ScalarEvolution` starts to keep them.
//!
//! # Example
//! `%x = add i32 %a, 1` has `(1 + %a)`. Its operand edited to 2 without a
//! `forget_value`, `verify` says: `%x: kept (1 + %a), now (2 + %a), differ by
//! -1`.
//!
//! # Used by
//! `indvars`, after each step, under `LLRM_VERIFY`.

use llrm_mir::module::{Operand, ValueDef, ValueId};

use super::expr::{Kind, Scev, Use};
use super::print::Labels;
use super::{Cx, Flags, ScalarEvolution};

impl ScalarEvolution {
    /// `SCEVMapper`: `s`, an expression of `self`, as the same expression of
    /// `into`, a new universe.
    fn mapped_into(
        &self,
        into: &mut ScalarEvolution,
        cx: &Cx,
        s: Scev,
    ) -> Scev {
        let operands = |this: &Self, into: &mut ScalarEvolution, ops: &[Use]| -> Vec<Use> {
            ops.iter().map(|op| Use::of(this.mapped_into(into, cx, op.scev))).collect()
        };
        match &self.node(s).kind {
            Kind::Constant(value) => into.get_constant(value.clone()),
            Kind::Unknown(value) => into.get_unknown(cx, *value),
            Kind::CouldNotCompute => into.get_could_not_compute(),
            Kind::PtrToAddr(op) => {
                let op = self.mapped_into(into, cx, op.scev);
                into.get_ptr_to_addr_expr(cx, op)
            }
            Kind::Truncate(op) => {
                let op = Use::of(self.mapped_into(into, cx, op.scev));
                into.get_truncate_expr(cx, op, self.type_of(s), 0)
            }
            Kind::ZeroExtend(op) => {
                let op = Use::of(self.mapped_into(into, cx, op.scev));
                into.get_zero_extend_expr(cx, op, self.type_of(s), 0)
            }
            Kind::SignExtend(op) => {
                let op = Use::of(self.mapped_into(into, cx, op.scev));
                into.get_sign_extend_expr(cx, op, self.type_of(s), 0)
            }
            Kind::UDiv(lhs, rhs) => {
                let (lhs, rhs) = (self.mapped_into(into, cx, lhs.scev), self.mapped_into(into, cx, rhs.scev));
                into.get_udiv_expr(cx, Use::of(lhs), Use::of(rhs))
            }
            Kind::Add(ops) => {
                let ops = operands(self, into, ops);
                into.get_add_expr(cx, ops, Flags::default(), 0).scev
            }
            Kind::Mul(ops) => {
                let ops = operands(self, into, ops);
                into.get_mul_expr(cx, ops, Flags::default(), 0).scev
            }
            Kind::AddRec(ops, header) => {
                let ops = operands(self, into, ops);
                into.get_add_rec_expr(cx, ops, *header, Flags::default()).scev
            }
            Kind::SMax(ops) | Kind::UMax(ops) | Kind::SMin(ops) | Kind::UMin(ops) => {
                let ops = operands(self, into, ops);
                into.get_min_max_expr(cx, self.node(s).kind.tag(), ops)
            }
            Kind::SequentialUMin(ops) => {
                let ops = operands(self, into, ops);
                into.get_sequential_min_max_expr(cx, self.node(s).kind.tag(), ops)
            }
        }
    }

    /// Whether every expression `self` holds is, up to what the pin lets
    /// pass, the one a new `ScalarEvolution` gives; the first that is not,
    /// said.
    pub fn verify(
        &self,
        cx: &Cx,
    ) -> Result<(), String> {
        let labels = Labels::of(cx.function);
        let mut fresh = ScalarEvolution::new();
        let mut cached: Vec<(ValueId, Scev)> = self.value_expr_map.iter().map(|(value, s)| (value, *s)).collect();
        cached.sort_by_key(|(value, _)| value.0);
        for (value, kept) in cached {
            let name = self.print_value_name(&labels, Operand::Value(value));
            // The reverse map names it.
            if !self.expr_value_map.get(&kept).is_some_and(|values| values.contains(&value)) {
                return Err(format!("{name} is in ValueExprMap but not in ExprValueMap"));
            }
            let ValueDef::Instruction(inst) = cx.function.value(value).def else { continue };
            if cx.function.parent(inst).is_none() {
                return Err(format!("{name}: kept {}, and the value is gone", self.print(cx, &labels, kept)));
            }
            let old = self.mapped_into(&mut fresh, cx, kept);
            let new = fresh.get_scev(cx, Operand::Value(value));
            if fresh.type_of(old) != fresh.type_of(new) {
                continue;
            }
            let delta = fresh.get_minus_scev(cx, Use::of(old), Use::of(new), 0, 0);
            if let Some(difference) = fresh.as_constant(delta)
                && !difference.is_zero()
            {
                return Err(format!(
                    "{name}: kept {}, now {}, differ by {}",
                    self.print(cx, &labels, kept),
                    fresh.print(cx, &labels, new),
                    difference.display()
                ));
            }
        }
        for (expr, values) in &self.expr_value_map {
            for value in values {
                match self.value_expr_map.get(value) {
                    None => return Err(format!("a value is in ExprValueMap but not in ValueExprMap")),
                    Some(mapped) if mapped != expr => return Err("a value is mapped to another expression".to_owned()),
                    Some(_) => {}
                }
            }
        }
        Ok(())
    }

    /// Whether each expression `self` holds prints, flags aside, as a new
    /// `ScalarEvolution` prints the value's: a held expression of a value whose
    /// operands changed since is stale, though `verify` lets a difference that
    /// is not a constant pass (the pin's check). The oracle of a kept SE
    /// (`Scevs::consistent`), not a translation of anything.
    pub fn verify_same(
        &self,
        cx: &Cx,
    ) -> Result<(), String> {
        let labels = Labels::of(cx.function);
        let mut fresh = ScalarEvolution::new();
        let bare =
            |text: String| ["<nuw>", "<nsw>", "<nusw>", "<nw>"].iter().fold(text, |text, flag| text.replace(flag, ""));
        let mut cached: Vec<(ValueId, Scev)> = self.value_expr_map.iter().map(|(value, s)| (value, *s)).collect();
        cached.sort_by_key(|(value, _)| value.0);
        for (value, kept) in cached {
            let ValueDef::Instruction(inst) = cx.function.value(value).def else { continue };
            if cx.function.parent(inst).is_none() {
                continue;
            }
            let was = bare(self.print(cx, &labels, kept));
            let new = fresh.get_scev(cx, Operand::Value(value));
            let now = bare(fresh.print(cx, &labels, new));
            if was != now && !weaker(&was, &now) {
                return Err(format!(
                    "{}: kept {was}, now {now}",
                    self.print_value_name(&labels, Operand::Value(value))
                ));
            }
        }
        Ok(())
    }
}

/// Whether `was` is `now` with some `umin_seq` that a fresh SE proved not
/// poison made a plain `umin`: the same value in the weaker form, which
/// lazily inferred flags make by when the expression was first made (the
/// pin's `insertValueToMap`, ScalarEvolution.cpp:4547: "It should be
/// equivalent, but may not necessarily be exactly the same"). The operators
/// and operands are compared as a bag, as `umin` reorders them.
fn weaker(
    was: &str,
    now: &str,
) -> bool {
    let seqs = |text: &str| text.matches(" umin_seq ").count();
    let bag = |text: &str| {
        let mut tokens: Vec<String> = text
            .replace(" umin_seq ", " umin ")
            .split_whitespace()
            .map(|token| token.trim_matches(['(', ')']).to_owned())
            .collect();
        tokens.sort_unstable();
        tokens
    };
    seqs(was) > seqs(now) && bag(was) == bag(now)
}

#[cfg(test)]
mod tests {
    use super::weaker;

    /// The oracle read a kept `umin_seq` that a fresh SE made `umin` as stale
    /// (queens -O3 under `LLRM_CHECK_PRESERVED`); the other way round, and
    /// any other difference, still is.
    #[test]
    fn test_only_a_kept_sequential_min_may_be_plain_now() {
        let seq = "(true + ((true + %32) umin_seq (true + %33) umin_seq (true + %35)))";
        let plain = "(true + (((true + %32) umin (true + %33)) umin_seq (true + %35)))";
        assert!(weaker(seq, plain));
        assert!(!weaker(plain, seq));
        assert!(!weaker(seq, seq));
        assert!(!weaker(seq, "(true + (((true + %32) umin (true + %34)) umin_seq (true + %35)))"));
    }
    use llrm_mir::datalayout::DataLayout;
    use llrm_mir::domtree::DomTree;
    use llrm_mir::loopinfo::LoopInfo;
    use llrm_mir::module::{Module, Operand};
    use llrm_mir::valuetracking::AssumptionCache;

    use super::super::{Cx, ScalarEvolution};

    const TEXT: &str = "define i32 @f(i32 %a) {
b0:
  %x = add i32 %a, 1
  %y = mul i32 %x, 3
  ret i32 %y
}
";

    /// `act` over the module's `cx`, with the module borrowed shared.
    fn with_cx<R>(
        module: &Module,
        act: impl FnOnce(&Cx) -> R,
    ) -> R {
        let (_, _, function) = module.functions().next().expect("a function");
        let layout = DataLayout::default();
        let tree = DomTree::of(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let assumptions = AssumptionCache::new(module, function);
        let cx = Cx {
            context: &module.context,
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            layout: &layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        act(&cx)
    }

    fn value_named(
        module: &Module,
        name: &str,
    ) -> Operand {
        let (_, _, function) = module.functions().next().expect("a function");
        let inst = function
            .layout()
            .iter()
            .flat_map(|&block| function.iter_block(block))
            .find(|&inst| {
                function
                    .instruction(inst)
                    .result
                    .is_some_and(|value| function.value(value).name.as_deref() == Some(name))
            })
            .expect("the value");
        Operand::Value(function.instruction(inst).result.expect("a value"))
    }

    /// A pass that edits `%x` and does not `forget_value` it leaves `%x` and
    /// `%y` with the old expressions: the kept `ScalarEvolution` is wrong and
    /// says so; after `forget_value` it is right again.
    #[test]
    fn a_missed_forget_is_caught_and_a_forget_mends_it() {
        let mut module = llrm_mir::parse::module(TEXT).expect("a module");
        let mut se = ScalarEvolution::new();
        let (x, y) = (value_named(&module, "x"), value_named(&module, "y"));
        with_cx(&module, |cx| {
            se.get_scev(cx, y);
            assert_eq!(se.verify(cx), Ok(()), "a fresh cache is right");
        });
        // %x = add %a, 2
        let two = {
            let ty = module.functions().next().unwrap().2.operand_type(&module.context, x).unwrap();
            Operand::Constant(module.context.int(ty, 2))
        };
        let inst = {
            let (_, _, function) = module.functions().next().unwrap();
            let Operand::Value(value) = x else { unreachable!() };
            let llrm_mir::module::ValueDef::Instruction(inst) = function.value(value).def else { unreachable!() };
            inst
        };
        let function = module.function_mut("f").expect("@f").1;
        function.set_operand(inst, 1, two);
        let stale = with_cx(&module, |cx| se.verify(cx));
        assert!(stale.as_ref().is_err_and(|why| why.contains("differ by")), "{stale:?}");
        with_cx(&module, |cx| {
            se.forget_value(cx, x);
            se.get_scev(cx, y);
            assert_eq!(se.verify(cx), Ok(()));
        });
    }
}
