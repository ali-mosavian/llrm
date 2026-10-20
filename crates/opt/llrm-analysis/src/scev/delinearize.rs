//! Delinearization: the subscripts and dimension sizes of an array access
//! from its SCEV, `Delinearization.cpp` at `d1106deb71cc`.
//!
//! # How it works
//! `delinearize` gathers the parametric terms of an access function (the
//! strides of its add recurrences, and the products of unknowns that multiply
//! one), divides them down to the array's dimension sizes
//! (`find_array_dimensions`, by [`ScalarEvolution::divide`]) and then divides
//! the access function by each size in turn: the quotient left over is the
//! next outer subscript, the remainder the subscript of this dimension
//! (`compute_access_functions`). Where no parameter shows, the fixed-size
//! heuristic reads the sizes off the constant strides
//! (`delinearize_fixed_size_array`). `validate_delinearization_result` proves
//! the subscripts in range and the offset free of overflow.
//! `get_index_expressions_from_gep` reads them from a GEP whose source type
//! is an array.
//!
//! # Deviations from LLVM
//! - D1: LLVM sorts `Terms` by pointer (`array_pod_sort`) before dropping
//!   duplicates; the order here is the arena index, which is the order the
//!   terms were made in. The sort by number of factors that follows is stable
//!   here and not in LLVM (`llvm::sort`), so equal terms stay in that order.
//! - D2: `SCEVCollectAddRecMultiplies` skips an unknown that wraps a call
//!   (`isa<CallInst>`); an unknown here is a value, and a call is the value an
//!   instruction of opcode call defines.
//!
//! # Example
//! The access function `{{{0,+,(8 * %m * %o)},+,(8 * %o)},+,8}` of
//! `A[i][j][k]` over `double A[n][m][o]` has the terms `(8 * %m * %o)` and
//! `(8 * %o)`; the sizes are `[%m][%o]` and 8 for the element, the subscripts
//! `{0,+,1}` in each loop.

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, IntPredicate, Opcode};
use llrm_mir::types::Type as MirType;

use super::expr::{Kind, Scev, Ty, Use};
use super::{Cx, Flags, ScalarEvolution};

impl ScalarEvolution {
    /// `SCEVTraversal::visitAll`: `follow` runs on each node as it is pushed,
    /// and a node it answers false for is not entered.
    pub fn visit_all(
        &mut self,
        root: Scev,
        follow: &mut dyn FnMut(&mut Self, Scev) -> bool,
    ) {
        let mut visited = llrm_mir::hash::HashSet::default();
        let mut worklist = Vec::new();
        if visited.insert(root) && follow(self, root) {
            worklist.push(root);
        }
        while let Some(s) = worklist.pop() {
            for operand in self.operands_of(s).to_vec() {
                if visited.insert(operand.scev) && follow(self, operand.scev) {
                    worklist.push(operand.scev);
                }
            }
        }
    }

    /// `containsUndefs`: whether `s` has an unknown that is `undef`.
    fn contains_undefs(
        &self,
        cx: &Cx,
        s: Scev,
    ) -> bool {
        self.expr_contains(s, &|expr| {
            matches!(
                self.node(expr).kind,
                Kind::Unknown(Operand::Constant(id)) if matches!(cx.context.get(id).kind, ConstantKind::Undef)
            )
        })
    }

    /// `collectParametricTerms`: the strides of the add recurrences of `expr`,
    /// and the products of unknowns that multiply an add recurrence.
    pub fn collect_parametric_terms(
        &mut self,
        cx: &Cx,
        expr: Scev,
        terms: &mut Vec<Scev>,
    ) {
        // SCEVCollectStrides.
        let mut strides = Vec::new();
        self.visit_all(expr, &mut |se, s| {
            let Kind::AddRec(operands, _) = se.node(s).kind.clone() else { return false };
            if operands.len() != 2 {
                return false;
            }
            strides.push(operands[1].scev);
            true
        });

        for stride in strides {
            // SCEVCollectTerms.
            self.visit_all(stride, &mut |se, s| {
                let kind = &se.node(s).kind;
                if matches!(kind, Kind::Unknown(_) | Kind::Mul(_) | Kind::SignExtend(_)) && !se.contains_undefs(cx, s) {
                    terms.push(s);
                }
                // Keep looking when S is a specific type expression.
                matches!(kind, Kind::Add(_) | Kind::AddRec(..))
            });
        }

        // SCEVCollectAddRecMultiplies.
        self.visit_all(expr, &mut |se, s| {
            if let Kind::Mul(factors) = se.node(s).kind.clone() {
                let mut has_add_rec = false;
                let mut operands = Vec::new();
                for factor in factors {
                    match se.node(factor.scev).kind.clone() {
                        Kind::Unknown(value) if !se.unknown_is_call(cx, value) => operands.push(factor),
                        Kind::Unknown(_) => has_add_rec = true,
                        _ => {
                            // SCEVHasAddRec.
                            let mut contains = false;
                            se.visit_all(factor.scev, &mut |se, inner| {
                                if matches!(se.node(inner).kind, Kind::AddRec(..)) {
                                    contains = true;
                                    return false;
                                }
                                true
                            });
                            has_add_rec |= contains;
                        }
                    }
                }
                if operands.is_empty() {
                    return true;
                }
                if !has_add_rec {
                    return false;
                }
                let product = se.get_mul_expr(cx, operands, Flags::default(), 0).scev;
                terms.push(product);
            }
            // Keep looking when S is a specific type expression.
            matches!(se.node(s).kind, Kind::Add(_) | Kind::AddRec(..))
        });
    }

    /// `isa<CallInst>(Unknown->getValue())` (D2).
    fn unknown_is_call(
        &self,
        cx: &Cx,
        value: Operand,
    ) -> bool {
        let Operand::Value(id) = value else { return false };
        let ValueDef::Instruction(inst) = cx.function.value(id).def else { return false };
        matches!(cx.function.instruction(inst).opcode, Opcode::Call(_))
    }

    /// `findArrayDimensionsRec`.
    fn find_array_dimensions_rec(
        &mut self,
        cx: &Cx,
        terms: &mut Vec<Scev>,
        sizes: &mut Vec<Scev>,
    ) -> bool {
        let last = terms.len() - 1;
        let mut step = terms[last];

        // End of recursion.
        if last == 0 {
            if let Kind::Mul(factors) = self.node(step).kind.clone() {
                let non_constant: Vec<Use> = factors
                    .into_iter()
                    .filter(|factor| !matches!(self.node(factor.scev).kind, Kind::Constant(_)))
                    .collect();
                step = self.get_mul_expr(cx, non_constant, Flags::default(), 0).scev;
            }
            sizes.push(step);
            return true;
        }

        for term in terms.iter_mut() {
            // Normalize the terms before the next call to
            // findArrayDimensionsRec.
            let (quotient, remainder) = self.divide(cx, *term, step);

            // Bail out when GCD does not evenly divide one of the terms.
            if !self.is_zero_scev(remainder) {
                return false;
            }
            *term = quotient;
        }

        // Remove all SCEVConstants.
        terms.retain(|&expr| !matches!(self.node(expr).kind, Kind::Constant(_)));

        if !terms.is_empty() && !self.find_array_dimensions_rec(cx, terms, sizes) {
            return false;
        }

        sizes.push(step);
        true
    }

    /// `containsParameters`: whether a term holds an unknown.
    fn contains_parameters(
        &self,
        terms: &[Scev],
    ) -> bool {
        terms.iter().any(|&term| self.expr_contains(term, &|s| matches!(self.node(s).kind, Kind::Unknown(_))))
    }

    /// `numberOfTerms`: the product terms in `s`.
    fn number_of_terms(
        &self,
        s: Scev,
    ) -> usize {
        match &self.node(s).kind {
            Kind::Mul(factors) => factors.len(),
            _ => 1,
        }
    }

    /// `removeConstantFactors`.
    fn remove_constant_factors(
        &mut self,
        cx: &Cx,
        t: Scev,
    ) -> Option<Scev> {
        match self.node(t).kind.clone() {
            Kind::Constant(_) => None,
            Kind::Unknown(_) => Some(t),
            Kind::Mul(factors) => {
                let rest: Vec<Use> = factors
                    .into_iter()
                    .filter(|factor| !matches!(self.node(factor.scev).kind, Kind::Constant(_)))
                    .collect();
                Some(self.get_mul_expr(cx, rest, Flags::default(), 0).scev)
            }
            _ => Some(t),
        }
    }

    /// `findArrayDimensions`: the dimension sizes the terms imply, the element
    /// size last; empty where they imply none.
    pub fn find_array_dimensions(
        &mut self,
        cx: &Cx,
        terms: &mut Vec<Scev>,
        sizes: &mut Vec<Scev>,
        element_size: Option<Scev>,
    ) {
        let Some(element_size) = element_size else { return };
        if terms.is_empty() {
            return;
        }

        // Early return when Terms do not contain parameters: we do not
        // delinearize non parametric SCEVs.
        if !self.contains_parameters(terms) {
            return;
        }

        // Remove duplicates (D1).
        terms.sort();
        terms.dedup();

        // Put larger terms first.
        let mut keyed: Vec<(usize, Scev)> = terms.iter().map(|&term| (self.number_of_terms(term), term)).collect();
        keyed.sort_by(|a, b| b.0.cmp(&a.0));
        *terms = keyed.into_iter().map(|(_, term)| term).collect();

        // Try to divide all terms by the element size. If term is not
        // divisible by element size, proceed with the original term.
        for term in terms.iter_mut() {
            let (quotient, _) = self.divide(cx, *term, element_size);
            if !self.is_zero_scev(quotient) {
                *term = quotient;
            }
        }

        // Remove constant factors.
        let mut new_terms: Vec<Scev> = Vec::new();
        for &term in terms.iter() {
            if let Some(new_term) = self.remove_constant_factors(cx, term) {
                new_terms.push(new_term);
            }
        }

        if new_terms.is_empty() || !self.find_array_dimensions_rec(cx, &mut new_terms, sizes) {
            sizes.clear();
            return;
        }

        // The last element to be pushed into Sizes is the size of an element.
        sizes.push(element_size);
    }

    /// `computeAccessFunctions`: the subscript of each dimension, the
    /// outermost first, or none where the access does not divide into `sizes`.
    pub fn compute_access_functions(
        &mut self,
        cx: &Cx,
        expr: Scev,
        subscripts: &mut Vec<Scev>,
        sizes: &mut Vec<Scev>,
    ) {
        // Early exit in case this SCEV is not an affine multivariate
        // function.
        if sizes.is_empty() {
            return;
        }
        if let Kind::AddRec(operands, _) = &self.node(expr).kind
            && operands.len() != 2
        {
            return;
        }

        // Clear output vector.
        subscripts.clear();

        let mut res = expr;
        let last = sizes.len() - 1;
        for i in (0..=last).rev() {
            let size = sizes[i];
            let (quotient, remainder) = self.divide(cx, res, size);
            res = quotient;

            // Do not record the last subscript corresponding to the size of
            // elements in the array.
            if i == last {
                // Bail out if the byte offset is non-zero.
                if !self.is_zero_scev(remainder) {
                    subscripts.clear();
                    sizes.clear();
                    return;
                }
                continue;
            }

            // Record the access function for the current subscript.
            subscripts.push(remainder);
        }

        // Also push in last position the remainder of the last division: it
        // will be the access function of the innermost dimension.
        subscripts.push(res);
        subscripts.reverse();
    }

    /// `delinearize`: split `expr` into subscripts and sizes, both empty where
    /// it cannot.
    pub fn delinearize(
        &mut self,
        cx: &Cx,
        expr: Scev,
        element_size: Option<Scev>,
    ) -> (Vec<Scev>, Vec<Scev>) {
        let mut subscripts = Vec::new();
        let mut sizes = Vec::new();

        // First step: collect parametric terms.
        let mut terms = Vec::new();
        self.collect_parametric_terms(cx, expr, &mut terms);
        if terms.is_empty() {
            return (subscripts, sizes);
        }

        // Second step: find subscript sizes.
        self.find_array_dimensions(cx, &mut terms, &mut sizes, element_size);
        if sizes.is_empty() {
            return (subscripts, sizes);
        }

        // Third step: compute the access functions for each subscript.
        self.compute_access_functions(cx, expr, &mut subscripts, &mut sizes);
        (subscripts, sizes)
    }

    /// `collectConstantAbsSteps`.
    fn collect_constant_abs_steps(
        &mut self,
        cx: &Cx,
        expr: Scev,
        steps: &mut Vec<u64>,
        element_size: u64,
    ) -> bool {
        // End of recursion. The constant value also must be a multiple of
        // ElementSize.
        if let Some(constant) = self.as_constant(expr) {
            return constant.zext_value() % u128::from(element_size) == 0;
        }

        let Kind::AddRec(operands, _) = self.node(expr).kind.clone() else { return false };
        if operands.len() != 2 {
            return false;
        }
        let step = operands[1].scev;
        let Some(step_value) = self.as_constant(step) else { return false };

        let magnitude = step_value.sext_value().unsigned_abs();
        if magnitude % u128::from(element_size) != 0 {
            return false;
        }
        // Bail out when the step is too large.
        let Ok(quotient) = u64::try_from(magnitude / u128::from(element_size)) else { return false };

        steps.push(quotient);
        self.collect_constant_abs_steps(cx, operands[0].scev, steps, element_size)
    }

    /// `findFixedSizeArrayDimensions`: the sizes of a fixed-size array read off
    /// the constant strides, the outermost first and the element size last.
    pub fn find_fixed_size_array_dimensions(
        &mut self,
        cx: &Cx,
        expr: Scev,
        sizes: &mut Vec<u64>,
        element_size: Option<Scev>,
    ) -> bool {
        let Some(element_size) = element_size else { return false };
        let Some(element_size_ap) = self.as_constant(element_size) else { return false };
        if element_size_ap.is_zero() {
            return false;
        }
        // Early exit when ElementSize is not a positive constant.
        let Ok(element_size_const) = u64::try_from(element_size_ap.zext_value()) else { return false };

        if !self.collect_constant_abs_steps(cx, expr, sizes, element_size_const) || sizes.is_empty() {
            sizes.clear();
            return false;
        }

        // At this point, Sizes contains the absolute step recurrences for all
        // induction variables; restore the length of each dimension by
        // dividing by the next smaller one.
        sizes.sort_by(|a, b| b.cmp(a));
        sizes.dedup();

        // The last element in Sizes should be ElementSize. At this point, all
        // values in Sizes are assumed to be divided by ElementSize, so
        // replace it with 1.
        debug_assert!(*sizes.last().expect("a size") != 0, "Unexpected zero size in Sizes.");
        *sizes.last_mut().expect("a size") = 1;

        for i in 0..sizes.len() - 1 {
            let previous = sizes[i + 1];
            if sizes[i] % previous != 0 {
                sizes.clear();
                return false;
            }
            sizes[i] /= previous;
        }

        // Finally, the last element in Sizes should be ElementSize.
        *sizes.last_mut().expect("a size") = element_size_const;
        true
    }

    /// `delinearizeFixedSizeArray`.
    pub fn delinearize_fixed_size_array(
        &mut self,
        cx: &Cx,
        expr: Scev,
        element_size: Option<Scev>,
    ) -> (Vec<Scev>, Vec<Scev>) {
        let mut subscripts = Vec::new();
        let mut sizes = Vec::new();

        // First step: find the fixed array size.
        let mut constant_sizes = Vec::new();
        if !self.find_fixed_size_array_dimensions(cx, expr, &mut constant_sizes, element_size) {
            return (subscripts, sizes);
        }

        // Convert the constant size to SCEV.
        let ty = self.type_of(expr);
        for size in constant_sizes {
            sizes.push(self.get_constant_of_type(cx, ty, size as i128));
        }

        // Second step: compute the access functions for each subscript.
        self.compute_access_functions(cx, expr, &mut subscripts, &mut sizes);
        (subscripts, sizes)
    }

    /// `validateDelinearizationResult`: the subscripts of the inner dimensions
    /// are in range and the offset they make does not overflow.
    pub fn validate_delinearization_result(
        &mut self,
        cx: &Cx,
        sizes: &[Scev],
        subscripts: &[Scev],
    ) -> bool {
        // Range check: 0 <= I_k < S_k for k = 2..n.
        for i in 1..sizes.len() {
            let mut size = sizes[i - 1];
            let mut subscript = subscripts[i];
            if !self.is_known_non_negative(cx, subscript) {
                return false;
            }

            let wider = self.get_wider_type(cx, self.type_of(subscript), self.type_of(size));
            subscript = self.get_noop_or_sign_extend(cx, subscript, wider);
            size = self.get_noop_or_sign_extend(cx, size, wider);
            if !self.is_known_predicate_scev(cx, IntPredicate::Slt, subscript, size) {
                return false;
            }
        }

        // The offset must not overflow: see the comment in
        // validateDelinearizationResult. I_1 can be negative, so Min is not
        // just 0.
        let one = self.get_one(cx, self.type_of(sizes[0]));
        let mut product = one;
        for &size in sizes {
            let Some(next) = self.mul_no_overflow(cx, product, size) else { return false };
            product = next;
        }
        let Some(min) = self.mul_no_overflow(cx, product, subscripts[0]) else { return false };

        // We have already checked that Min and Prod don't overflow, so it's
        // enough to check whether Min + Prod - 1 doesn't overflow.
        if !self.will_not_overflow(cx, BinaryOp::Add, true, min, product, None) {
            return false;
        }
        let max_plus_one = self.get_add_expr(cx, vec![Use::of(min), Use::of(product)], Flags::default(), 0).scev;
        let one = self.get_one(cx, self.type_of(max_plus_one));
        self.will_not_overflow(cx, BinaryOp::Sub, true, max_plus_one, one, None)
    }

    /// `MulOverflow` of `validateDelinearizationResult`.
    fn mul_no_overflow(
        &mut self,
        cx: &Cx,
        a: Scev,
        b: Scev,
    ) -> Option<Scev> {
        if !self.will_not_overflow(cx, BinaryOp::Mul, true, a, b, None) {
            return None;
        }
        Some(self.get_mul_expr(cx, vec![Use::of(a), Use::of(b)], Flags::default(), 0).scev)
    }

    /// `getIndexExpressionsFromGEP`: the indices and the array sizes of a GEP
    /// over nested arrays; empty where the source type is not one.
    pub fn get_index_expressions_from_gep(
        &mut self,
        cx: &Cx,
        gep: InstId,
    ) -> (Vec<Scev>, Vec<Scev>) {
        let mut subscripts = Vec::new();
        let mut sizes = Vec::new();
        let instruction = cx.function.instruction(gep);
        let Opcode::GetElementPtr { source } = instruction.opcode else { unreachable!("a getelementptr") };
        let pointer_ty = cx
            .function
            .operand_type(cx.context, instruction.operands[0])
            .and_then(|id| self.ty_of(cx, id))
            .expect("a pointer operand");
        let index_ty = self.get_effective_scev_type(cx, pointer_ty);
        let mut ty = source;
        let mut dropped_first_dim = false;
        for (i, &index) in instruction.operands.iter().enumerate().skip(1) {
            let expr = self.get_scev(cx, index);
            if i == 1 {
                if self.is_zero_scev(expr) {
                    dropped_first_dim = true;
                    continue;
                }
                subscripts.push(expr);
                continue;
            }

            let MirType::Array { element, count } = *cx.context.types.get(ty) else {
                return (Vec::new(), Vec::new());
            };

            subscripts.push(expr);
            if !(dropped_first_dim && i == 2) {
                sizes.push(self.get_constant_of_type(cx, index_ty, count as i128));
            }
            ty = element;
        }
        (subscripts, sizes)
    }

    /// `ScalarEvolution::getElementSize`: the allocation size of the type a
    /// load or store moves, as a constant of pointer width.
    pub fn get_element_size(
        &mut self,
        cx: &Cx,
        inst: InstId,
    ) -> Scev {
        let instruction = cx.function.instruction(inst);
        let ty = match instruction.opcode {
            Opcode::Load { .. } => instruction.ty,
            Opcode::Store { .. } => {
                cx.function.operand_type(cx.context, instruction.operands[0]).expect("a stored value has a type")
            }
            _ => unreachable!("a load or a store"),
        };
        let pointer = self.get_effective_scev_type(cx, Ty::Ptr(0));
        let size = cx.layout.alloc_size(&cx.context.types, ty);
        self.get_constant_of_type(cx, pointer, size as i128)
    }
}

/// `print<delinearization>` (`printDelinearization`) for each function of
/// `module` that has a body. The `Inst:` line is the access's place, `%name`
/// for a load and `store #n` for the n-th store of the function.
pub fn print_module_delinearization(module: &llrm_mir::module::Module) -> String {
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
        out +=
            &format!("Printing analysis 'Delinearization' for function '{}':", global.name.as_deref().unwrap_or("?"));
        for (block, inst) in function.walk() {
            let instruction = function.instruction(inst);
            let pointer = match instruction.opcode {
                Opcode::Load { .. } => instruction.operands[0],
                Opcode::Store { .. } => instruction.operands[1],
                _ => continue,
            };
            // Only delinearize the memory access in the innermost loop. Do not
            // analyze memory accesses outside loops.
            let Some(innermost) = loops.get_loop_for(block) else { continue };

            let access = se.get_scev_at_scope_of_value(&cx, pointer, Some(innermost.get_header())).scev;
            let base = se.get_pointer_base(access);
            // Do not delinearize if we cannot find the base pointer.
            if !matches!(se.node(base).kind, Kind::Unknown(_)) {
                break;
            }
            let access = se.get_minus_scev(&cx, Use::of(access), Use::of(base), 0, 0);

            out += "\n";
            out += &format!(
                "Inst:{}\n",
                instruction
                    .result
                    .map_or_else(|| "store".to_owned(), |result| se.print_value_name(&labels, Operand::Value(result)),)
            );
            out += &format!("AccessFunction: {}\n", se.print(&cx, &labels, access));

            let element = se.get_element_size(&cx, inst);
            let (mut subscripts, mut sizes) = se.delinearize(&cx, access, Some(element));
            let failed = |subscripts: &[Scev], sizes: &[Scev]| {
                subscripts.is_empty() || sizes.is_empty() || subscripts.len() != sizes.len()
            };
            if failed(&subscripts, &sizes) {
                (subscripts, sizes) = se.delinearize_fixed_size_array(&cx, access, Some(element));
            }
            if failed(&subscripts, &sizes) {
                out += "failed to delinearize\n";
                continue;
            }

            out += &format!("Base offset: {}\n", se.print(&cx, &labels, base));
            out += "ArrayDecl[UnknownSize]";
            for &size in &sizes[..sizes.len() - 1] {
                out += &format!("[{}]", se.print(&cx, &labels, size));
            }
            out += &format!(" with elements of {} bytes.\n", se.print(&cx, &labels, sizes[sizes.len() - 1]));
            out += "ArrayRef";
            for &subscript in &subscripts {
                out += &format!("[{}]", se.print(&cx, &labels, subscript));
            }
            out += "\n";
            let valid = se.validate_delinearization_result(&cx, &sizes, &subscripts);
            out += &format!("Delinearization validation: {}\n", if valid { "Succeeded" } else { "Failed" });
        }
    }
    out
}
