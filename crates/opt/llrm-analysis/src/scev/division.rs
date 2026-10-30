//! `SCEVDivision`: a quotient and a remainder of one expression by another,
//! `ScalarEvolutionDivision.cpp` at `d1106deb71cc`.
//!
//! # How it works
//! [`ScalarEvolution::divide`] finds `Q` and `R` with `N = D * Q + R`, not the
//! only pair: `(0, N)` always holds. Constants divide signed; sums and add
//! recurrences divide operand by operand; a product divides when one factor
//! does, or, when `D` is an unknown, by replacing `D` with 0 and then 1 in `N`.
//! Every other expression, and every division that fails, answers `(0, N)`.
//!
//! # Deviations from LLVM
//! - D1: a constant numerator over a constant zero is `(0, N)`, where
//!   `APInt::sdivrem` is undefined.
//! - D2: `SCEVVScale` has no node here, so its `cannotDivide` is the default
//!   for every kind not listed.
//!
//! # Example
//! `(8 * %m * %o) / (%m * %o)` is `(8, 0)`: the denominator is a product, so
//! `%m` divides first and `%o` after.

use llrm_mir::hash::HashMap;
use llrm_mir::module::Operand;

use super::expr::{Kind, Scev, Use};
use super::{Cx, Flags, ScalarEvolution};

impl ScalarEvolution {
    /// `SCEV::isZero`.
    pub fn is_zero_scev(
        &self,
        s: Scev,
    ) -> bool {
        self.as_constant(s).is_some_and(|value| value.is_zero())
    }

    /// `SCEV::isOne`.
    pub fn is_one_scev(
        &self,
        s: Scev,
    ) -> bool {
        self.as_constant(s).is_some_and(|value| value.is_one())
    }

    /// `SCEVDivision::divide`: the quotient and the remainder of
    /// `numerator / denominator`.
    pub fn divide(
        &mut self,
        cx: &Cx,
        numerator: Scev,
        denominator: Scev,
    ) -> (Scev, Scev) {
        debug_assert_eq!(self.type_of(numerator), self.type_of(denominator), "same type");
        let ty = self.type_of(denominator);
        let zero = self.get_zero(cx, ty);
        let one = self.get_one(cx, ty);

        // Check for the trivial case here to avoid having to check for it in
        // the rest of the code.
        if numerator == denominator {
            return (one, zero);
        }
        if self.is_zero_scev(numerator) {
            return (zero, zero);
        }
        // A simple case when N/1. The quotient is N.
        if denominator == one {
            return (numerator, zero);
        }

        // Split the Denominator when it is a product.
        if let Kind::Mul(factors) = self.node(denominator).kind.clone() {
            let mut quotient = numerator;
            for factor in factors {
                let (q, r) = self.divide(cx, quotient, factor.scev);
                quotient = q;
                // Bail out when the Numerator is not divisible by one of the
                // terms of the Denominator.
                if !self.is_zero_scev(r) {
                    return (zero, numerator);
                }
            }
            return (quotient, zero);
        }

        match self.node(numerator).kind.clone() {
            Kind::Constant(value) => self.divide_constant(cx, value, denominator, numerator),
            Kind::AddRec(operands, header) => self.divide_add_rec(cx, numerator, &operands, header, denominator),
            Kind::Add(operands) => self.divide_add(cx, numerator, &operands, denominator),
            Kind::Mul(operands) => self.divide_mul(cx, numerator, &operands, denominator),
            _ => (zero, numerator),
        }
    }

    /// `SCEVDivision::visitConstant`.
    fn divide_constant(
        &mut self,
        cx: &Cx,
        value: super::ap::Ap,
        denominator: Scev,
        numerator: Scev,
    ) -> (Scev, Scev) {
        let Some(divisor) = self.as_constant(denominator) else {
            return (self.get_zero(cx, self.type_of(denominator)), numerator);
        };
        // D1: a zero divisor is undefined behaviour in `APInt::sdivrem`.
        if divisor.is_zero() {
            return (self.get_zero(cx, self.type_of(denominator)), numerator);
        }
        let quotient = self.get_constant(value.sdiv(divisor));
        let remainder = self.get_constant(value.srem(divisor));
        (quotient, remainder)
    }

    /// `SCEVDivision::visitAddRecExpr`.
    fn divide_add_rec(
        &mut self,
        cx: &Cx,
        numerator: Scev,
        operands: &[Use],
        header: llrm_mir::module::BlockId,
        denominator: Scev,
    ) -> (Scev, Scev) {
        let ty = self.type_of(denominator);
        let zero = self.get_zero(cx, ty);
        if operands.len() != 2 {
            return (zero, numerator);
        }
        let start = operands[0].scev;
        let step = self.get_step_recurrence(cx, numerator);
        let (start_q, start_r) = self.divide(cx, start, denominator);
        let (step_q, step_r) = self.divide(cx, step, denominator);
        // Bail out if the types do not match.
        if [start_q, start_r, step_q, step_r].iter().any(|&one| self.type_of(one) != ty) {
            return (zero, numerator);
        }
        let quotient = self.get_add_rec_expr_two(cx, Use::of(start_q), Use::of(step_q), header, Flags::default());
        let remainder = self.get_add_rec_expr_two(cx, Use::of(start_r), Use::of(step_r), header, Flags::default());
        (quotient.scev, remainder.scev)
    }

    /// `SCEVDivision::visitAddExpr`.
    fn divide_add(
        &mut self,
        cx: &Cx,
        numerator: Scev,
        operands: &[Use],
        denominator: Scev,
    ) -> (Scev, Scev) {
        let ty = self.type_of(denominator);
        let zero = self.get_zero(cx, ty);
        let mut quotients = Vec::new();
        let mut remainders = Vec::new();
        for operand in operands {
            let (q, r) = self.divide(cx, operand.scev, denominator);
            // Bail out if types do not match.
            if self.type_of(q) != ty || self.type_of(r) != ty {
                return (zero, numerator);
            }
            quotients.push(Use::of(q));
            remainders.push(Use::of(r));
        }
        if quotients.len() == 1 {
            return (quotients[0].scev, remainders[0].scev);
        }
        let quotient = self.get_add_expr(cx, quotients, Flags::default(), 0).scev;
        let remainder = self.get_add_expr(cx, remainders, Flags::default(), 0).scev;
        (quotient, remainder)
    }

    /// `SCEVDivision::visitMulExpr`.
    fn divide_mul(
        &mut self,
        cx: &Cx,
        numerator: Scev,
        operands: &[Use],
        denominator: Scev,
    ) -> (Scev, Scev) {
        let ty = self.type_of(denominator);
        let zero = self.get_zero(cx, ty);
        let one = self.get_one(cx, ty);
        let mut quotients = Vec::new();

        let mut found_denominator_term = false;
        for operand in operands {
            // Bail out if types do not match.
            if self.type_of(operand.scev) != ty {
                return (zero, numerator);
            }
            if found_denominator_term {
                quotients.push(*operand);
                continue;
            }

            // Check whether Denominator divides one of the product operands.
            let (q, r) = self.divide(cx, operand.scev, denominator);
            if !self.is_zero_scev(r) {
                quotients.push(*operand);
                continue;
            }

            // Bail out if types do not match.
            if self.type_of(q) != ty {
                return (zero, numerator);
            }
            found_denominator_term = true;
            quotients.push(Use::of(q));
        }

        if found_denominator_term {
            let quotient = if quotients.len() == 1 {
                quotients[0].scev
            } else {
                self.get_mul_expr(cx, quotients, Flags::default(), 0).scev
            };
            return (quotient, zero);
        }

        let Kind::Unknown(value) = self.node(denominator).kind.clone() else {
            return (zero, numerator);
        };

        // The Remainder is obtained by replacing Denominator by 0 in Numerator.
        let mut rewrites = HashMap::default();
        rewrites.insert(value, zero);
        let remainder = self.rewrite_parameters(cx, numerator, &rewrites);

        if self.is_zero_scev(remainder) {
            // The Quotient is obtained by replacing Denominator by 1 in
            // Numerator.
            rewrites.insert(value, one);
            return (self.rewrite_parameters(cx, numerator, &rewrites), remainder);
        }

        // Quotient is (Numerator - Remainder) divided by Denominator.
        let diff = self.get_minus_scev(cx, Use::of(numerator), Use::of(remainder), 0, 0);
        // This SCEV does not seem to simplify: fail the division here.
        if self.size_of_scev(diff) > self.size_of_scev(numerator) {
            return (zero, numerator);
        }
        let (q, r) = self.divide(cx, diff, denominator);
        if r != zero {
            return (zero, numerator);
        }
        (q, remainder)
    }

    /// `SCEVParameterRewriter::rewrite`: `s` with each unknown in `map`
    /// replaced.
    pub fn rewrite_parameters(
        &mut self,
        cx: &Cx,
        s: Scev,
        map: &HashMap<Operand, Scev>,
    ) -> Scev {
        if let Kind::Unknown(value) = self.node(s).kind {
            return map.get(&value).copied().unwrap_or(s);
        }
        self.rewrite_operands(cx, s, &mut |se, operand| se.rewrite_parameters(cx, operand, map))
    }

    /// `sizeOfSCEV`: the nodes `SCEVTraversal` reaches from `s`, each once.
    pub fn size_of_scev(
        &self,
        s: Scev,
    ) -> usize {
        let mut visited = llrm_mir::hash::HashSet::default();
        visited.insert(s);
        let mut worklist = vec![s];
        while let Some(one) = worklist.pop() {
            for operand in self.operands_of(one) {
                if visited.insert(operand.scev) {
                    worklist.push(operand.scev);
                }
            }
        }
        visited.len()
    }
}

/// `print<scev-division>` (`SCEVDivisionPrinterPass::runImpl`) for each
/// function of `module` that has a body: its signed divisions, numerator,
/// denominator, quotient and remainder. The instruction itself is its place
/// in the function, `%name` as the function numbers its values.
pub fn print_module_division(module: &llrm_mir::module::Module) -> String {
    use llrm_mir::opcode::{BinaryOp, Opcode};

    let layout = module
        .datalayout
        .as_deref()
        .and_then(|text| llrm_mir::datalayout::DataLayout::parse(text).ok())
        .unwrap_or_default();
    let mut out = String::new();
    for (_, global, function) in module.functions() {
        if function.layout().is_empty() {
            continue;
        }
        let tree = llrm_mir::domtree::DomTree::of(function);
        let loops = llrm_mir::loopinfo::LoopInfo::analyze(function, || &tree);
        let assumptions = llrm_mir::valuetracking::AssumptionCache::new(module, function);
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
        let labels = super::print::Labels::of(function);
        let mut se = ScalarEvolution::new();
        out += &format!(
            "Printing analysis 'Scalar Evolution Division' for function '{}':\n",
            global.name.as_deref().unwrap_or("?")
        );
        for (_, inst) in function.walk() {
            let instruction = function.instruction(inst);
            if instruction.opcode != Opcode::Binary(BinaryOp::SDiv) {
                continue;
            }
            let numerator = se.get_scev(&cx, instruction.operands[0]);
            let denominator = se.get_scev(&cx, instruction.operands[1]);
            let (quotient, remainder) = se.divide(&cx, numerator, denominator);
            let value = Operand::Value(instruction.result.expect("a division's value"));
            out += &format!("Instruction: {}\n", se.print_value_name(&labels, value));
            for (name, s) in [
                ("Numerator", numerator),
                ("Denominator", denominator),
                ("Quotient", quotient),
                ("Remainder", remainder),
            ] {
                out += &format!("  {name}: {}\n", se.print(&cx, &labels, s));
            }
        }
    }
    out
}
