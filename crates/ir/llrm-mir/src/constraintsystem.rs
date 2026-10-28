//! `ConstraintSystem`: a system of linear constraints over integer variables,
//! decided by Fourier-Motzkin elimination.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ConstraintSystem.h` and
//! `ConstraintSystem.cpp`, each function linking the lines it translates. A
//! row is a constraint `c >= v1 * c1 + ... + vn * cn`, stored sparsely as
//! `(coefficient, variable)` pairs in ascending variable order, the constant
//! `c` being variable 0. `may_have_solution` removes the last variable by
//! combining each row that bounds it above with each that bounds it below
//! (Pugh's Omega-test tricks, as LLVM's), and gives up, saying "may have a
//! solution", where the system passes 500 rows. `is_condition_implied` adds
//! the negation of a row and asks whether any solution is left;
//! `get_sub_system` cuts the system to the rows connected to the query first.
//!
//! DEVIATE (anything Rust or MIR lacks):
//! 1. The variables are keyed by any `K: Copy + Eq + Hash`, not `Value *`: the
//!    system names none of its variables' properties.
//! 2. `dump` and `getVarNamesList` are `LLVM_DEBUG` output and are not
//!    translated.
//! 3. `-1 * LowerLast` is C++ signed arithmetic with no overflow check; it
//!    wraps here (only `i64::MIN` differs, which LLVM's callers never build:
//!    `decompose` bounds every coefficient).
//!
//! # Example
//! `x >= 1` and `y >= x` imply `y >= 1`: with rows `{-1, -1x}` and `{0, 1x,
//! -1y}`, adding the negation of `y >= 1` leaves no solution.
//!
//! # Used by
//! Nothing yet: `difference` moves here (ConstraintElimination).

use std::hash::Hash;

use crate::hash::HashMap;

/// `ConstraintSystem::Entry`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ConstraintSystem.h#L23>
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub coefficient: i64,
    pub id: u16,
}

impl Entry {
    pub fn new(
        coefficient: i64,
        id: u16,
    ) -> Self {
        Self { coefficient, id }
    }
}

/// A single constraint of the form `c >= v1 * c1 + ... + vn * cn`: `RowTy`.
pub type Row = Vec<Entry>;

/// `ConstraintSystem`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ConstraintSystem.h#L21>
#[derive(Clone, Debug)]
pub struct ConstraintSystem<K> {
    /// Number of variables in the system, not counting the constant part. The
    /// variables use the indices 1 to `num_variables`.
    num_variables: usize,
    /// Each entry represents a constraint like `c0 >= v0 * c1 + .... + v{n-1} *
    /// cn`.
    constraints: Vec<Row>,
    /// The variables and their index in the system.
    value2index: HashMap<K, u32>,
}

impl<K> Default for ConstraintSystem<K> {
    fn default() -> Self {
        Self { num_variables: 0, constraints: Vec::new(), value2index: HashMap::default() }
    }
}

/// `getLastCoefficient`.
#[inline(always)]
fn last_coefficient(
    row: &[Entry],
    id: u16,
) -> i64 {
    match row.last() {
        Some(last) if last.id == id => last.coefficient,
        _ => 0,
    }
}

/// `hasConstantEntry`: `row` has an entry for the constant part.
fn has_constant_entry(row: &[Entry]) -> bool {
    row.first().is_some_and(|first| first.id == 0)
}

/// `isConstantOnly`: `row` has no entry for any variable, i.e. it is of the
/// form `c >= 0`.
fn is_constant_only(row: &[Entry]) -> bool {
    row.is_empty() || (row.len() == 1 && row[0].id == 0)
}

/// `getConstant`: the constant part of `row`, 0 if it has no entry for it.
fn constant(row: &[Entry]) -> i64 {
    if has_constant_entry(row) { row[0].coefficient } else { 0 }
}

impl<K: Copy + Eq + Hash> ConstraintSystem<K> {
    pub fn new() -> Self {
        Self::default()
    }

    /// `ConstraintSystem(ArrayRef<Value *> FunctionArgs)`.
    pub fn with_arguments(arguments: &[K]) -> Self {
        let mut system = Self::default();
        system.num_variables += arguments.len();
        for argument in arguments {
            let next = system.value2index.len() as u32 + 1;
            system.value2index.entry(*argument).or_insert(next);
        }
        system
    }

    /// `ConstraintSystem(const DenseMap<Value *, unsigned> &Value2Index)`.
    pub fn with_variables(value2index: HashMap<K, u32>) -> Self {
        Self { num_variables: value2index.len(), constraints: Vec::new(), value2index }
    }

    /// `addRow`: false for a row that holds no variable.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ConstraintSystem.h#L84>
    pub fn add_row(
        &mut self,
        row: &[Entry],
        num_vars: usize,
    ) -> bool {
        // If all variable coefficients are 0, the constraint does not provide
        // any usable information.
        if is_constant_only(row) {
            return false;
        }
        debug_assert!(num_vars >= row[row.len() - 1].id as usize, "NumVars must cover all variables in R");
        self.num_variables = num_vars.max(self.num_variables);
        // Only keep non-zero coefficients; in particular drop the entry for
        // the constant part if it is 0.
        self.constraints.push(row.iter().copied().filter(|entry| entry.coefficient != 0).collect());
        true
    }

    pub fn value2index(&self) -> &HashMap<K, u32> {
        &self.value2index
    }

    pub fn value2index_mut(&mut self) -> &mut HashMap<K, u32> {
        &mut self.value2index
    }

    /// `negate`: the negation of `row`, empty on overflow.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ConstraintSystem.h#L117>
    pub fn negate(mut row: Row) -> Row {
        assert!(has_constant_entry(&row), "row must have a constant entry");
        // The negated constraint R is obtained by multiplying by -1 and adding
        // 1 to the constant.
        let Some(constant) = row[0].coefficient.checked_add(1) else { return Row::new() };
        row[0].coefficient = constant;
        Self::negate_or_equal(row)
    }

    /// `negateOrEqual`: every coefficient times -1, empty on overflow.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ConstraintSystem.h#L128>
    pub fn negate_or_equal(mut row: Row) -> Row {
        // The negated constraint R is obtained by multiplying by -1.
        for entry in &mut row {
            let Some(negated) = entry.coefficient.checked_mul(-1) else { return Row::new() };
            entry.coefficient = negated;
        }
        row
    }

    /// `toStrictLessThan`: `row` as a strict inequality, empty on overflow.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ConstraintSystem.h#L145>
    pub fn to_strict_less_than(mut row: Row) -> Row {
        assert!(has_constant_entry(&row), "row must have a constant entry");
        // The strict less than is obtained by subtracting 1 from the constant.
        let Some(constant) = row[0].coefficient.checked_sub(1) else { return Row::new() };
        row[0].coefficient = constant;
        row
    }

    pub fn last_constraint(&self) -> &Row {
        self.constraints.last().expect("Constraint system is empty")
    }

    pub fn pop_last_constraint(&mut self) {
        self.constraints.pop();
    }

    pub fn pop_last_n_variables(
        &mut self,
        n: usize,
    ) {
        assert!(self.num_variables >= n);
        self.num_variables -= n;
    }

    /// The number of rows in the system.
    pub fn size(&self) -> usize {
        self.constraints.len()
    }

    /// `eliminateUsingFM`: removes the last variable by Fourier-Motzkin
    /// elimination, with some tricks from Pugh, "The Omega test: a fast and
    /// practical integer programming algorithm for dependence analysis"
    /// (Supercomputing '91). False where the new system got too big.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstraintSystem.cpp#L22>
    fn eliminate_using_fm(&mut self) -> bool {
        assert!(!self.constraints.is_empty(), "should only be called for non-empty constraint systems");
        let last_idx = self.num_variables as u16;

        // First, either remove the variable in place if it is 0 or add the row
        // to RemainingRows and remove it from the system.
        let mut remaining_rows: Vec<Row> = Vec::new();
        let mut r1 = 0;
        while r1 < self.constraints.len() {
            if last_coefficient(&self.constraints[r1], last_idx) == 0 {
                let row = &mut self.constraints[r1];
                if row.last().is_some_and(|last| last.id == last_idx) {
                    row.pop();
                }
                r1 += 1;
            } else {
                let last = self.constraints.len() - 1;
                self.constraints.swap(r1, last);
                remaining_rows.push(self.constraints.pop().expect("a row"));
            }
        }

        // Process rows where the variable is != 0.
        let num_remaining_constraints = remaining_rows.len();
        for r1 in 0..num_remaining_constraints {
            for r2 in r1 + 1..num_remaining_constraints {
                // Examples of constraints stored as {Constant, Coeff_x,
                // Coeff_y} R1:  0 >=  1 * x + (-2) * y  => { 0,
                // 1, -2 } R2:  3 >=  2 * x +  3 * y    => { 3,
                // 2,  3 } LastIdx = 2 (tracking coefficient of
                // y) UpperLast: 3
                // LowerLast: -2
                let mut upper_last = last_coefficient(&remaining_rows[r2], last_idx);
                let mut lower_last = last_coefficient(&remaining_rows[r1], last_idx);
                debug_assert!(
                    upper_last != 0 && lower_last != 0,
                    "RemainingRows should only contain rows where the variable is != 0"
                );

                if (lower_last < 0 && upper_last < 0) || (lower_last > 0 && upper_last > 0) {
                    continue;
                }

                let (mut lower_r, mut upper_r) = (r1, r2);
                if upper_last < 0 {
                    std::mem::swap(&mut lower_r, &mut upper_r);
                    std::mem::swap(&mut lower_last, &mut upper_last);
                }

                let mut new_row = Row::new();
                let (mut idx_upper, mut idx_lower) = (0, 0);
                let (lower_row, upper_row) = (&remaining_rows[lower_r], &remaining_rows[upper_r]);
                // Combine the two rows to eliminate the variable. If any
                // coefficient computation overflows, skip them.
                let mut overflow = false;
                // Update constant and coefficients of both constraints. Stops
                // until every coefficient is updated or overflows.
                loop {
                    if idx_upper >= upper_row.len() || idx_lower >= lower_row.len() {
                        break;
                    }
                    // Starts with index 0 and updates every coefficients.
                    let (mut upper_v, mut lower_v) = (0_i64, 0_i64);
                    let mut current_id = u16::MAX;
                    if idx_upper < upper_row.len() {
                        current_id = upper_row[idx_upper].id.min(current_id);
                    }
                    if idx_lower < lower_row.len() {
                        current_id = lower_row[idx_lower].id.min(current_id);
                    }

                    if idx_upper < upper_row.len() && upper_row[idx_upper].id == current_id {
                        upper_v = upper_row[idx_upper].coefficient;
                        idx_upper += 1;
                    }

                    let Some(m1) = upper_v.checked_mul(lower_last.wrapping_mul(-1)) else {
                        overflow = true;
                        break;
                    };
                    if idx_lower < lower_row.len() && lower_row[idx_lower].id == current_id {
                        lower_v = lower_row[idx_lower].coefficient;
                        idx_lower += 1;
                    }

                    let Some(m2) = lower_v.checked_mul(upper_last) else {
                        overflow = true;
                        break;
                    };
                    // This algorithm is a variant of sparse Gaussian
                    // elimination.
                    //
                    // The new coefficient for CurrentId is
                    // N = UpperV * (-1) * LowerLast + LowerV * UpperLast
                    //
                    // UpperRow: { 3,  2,  3 }, LowerLast: -2
                    // LowerRow: { 0,  1, -2 }, UpperLast: 3
                    //
                    // After multiplication:
                    // UpperRow: { 6, 4, 6 }
                    // LowerRow: { 0, 3, -6 }
                    //
                    // Eliminates y after addition:
                    // N: { 6, 7, 0 } => 6 >= 7 * x
                    let Some(n) = m1.checked_add(m2) else {
                        overflow = true;
                        break;
                    };
                    // Skip variable that is completely eliminated.
                    if n == 0 {
                        continue;
                    }
                    new_row.push(Entry::new(n, current_id));
                }
                if overflow || new_row.is_empty() {
                    continue;
                }
                self.constraints.push(new_row);
                // Give up if the new system gets too big.
                if self.constraints.len() > 500 {
                    return false;
                }
            }
        }
        self.num_variables -= 1;
        true
    }

    /// `mayHaveSolutionImpl`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstraintSystem.cpp#L136>
    fn may_have_solution_impl(&mut self) -> bool {
        while !self.constraints.is_empty() && self.num_variables > 0 {
            if !self.eliminate_using_fm() {
                return true;
            }
        }
        debug_assert!(
            self.constraints.is_empty() || self.num_variables == 0,
            "non-empty system must have all variables eliminated"
        );
        self.constraints.iter().all(|row| constant(row) >= 0)
    }

    /// `mayHaveSolution`: whether there may be a solution for the constraints
    /// in the system. The system is left eliminated.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstraintSystem.cpp#L183>
    pub fn may_have_solution(&mut self) -> bool {
        self.may_have_solution_impl()
    }

    /// `getSubSystem`: the sub-system of constraints connected (transitively)
    /// to the query `row`, with variables compacted to a dense index range,
    /// and `row` translated to it.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstraintSystem.cpp#L192>
    pub fn get_sub_system(
        &self,
        row: &[Entry],
    ) -> (ConstraintSystem<K>, Row) {
        debug_assert!(
            row.last().is_none_or(|last| last.id as usize <= self.num_variables),
            "query must only use variables of the system"
        );
        // Only constraints that share a variable (transitively) with a query R
        // can affect whether system + !R has a solution.
        //
        // Mark variables in the query and collect to the transitive closure
        // over variables that co-occur in a constraint row.
        let mut sub_system = ConstraintSystem::<K>::default();
        let mut in_system = vec![false; self.num_variables + 1];
        for entry in row {
            if entry.id != 0 {
                in_system[entry.id as usize] = true;
            }
        }
        let shares_variable =
            |in_system: &[bool], row: &[Entry]| row.iter().any(|e| e.id != 0 && in_system[e.id as usize]);
        let mut changed = true;
        while changed {
            changed = false;
            for constraint in &self.constraints {
                // No common variables, skip.
                if !shares_variable(&in_system, constraint) {
                    continue;
                }
                for entry in constraint {
                    if entry.id != 0 && !in_system[entry.id as usize] {
                        in_system[entry.id as usize] = true;
                        changed = true;
                    }
                }
            }
        }

        // Assign compact indices to the variables of the sub-system.
        let mut old_to_new = vec![0_u32; self.num_variables + 1];
        let mut next_idx = 1_u32;
        for id in 0..in_system.len() {
            if in_system[id] {
                old_to_new[id] = next_idx;
                next_idx += 1;
            }
        }

        // Build new compact set of rows.
        sub_system.num_variables = next_idx as usize - 1;
        for constraint in &self.constraints {
            if !shares_variable(&in_system, constraint) {
                continue;
            }
            let mut new_row = Row::new();
            for entry in constraint {
                let new = old_to_new[entry.id as usize];
                debug_assert!((entry.id == 0) == (new == 0), "constant entry must be preserved");
                new_row.push(Entry::new(entry.coefficient, new as u16));
            }
            sub_system.constraints.push(new_row);
        }

        // Remap the query row into the component's compact index space.
        let mut new_r = vec![Entry::new(constant(row), 0)];
        for entry in row {
            if entry.id != 0 {
                new_r.push(Entry::new(entry.coefficient, old_to_new[entry.id as usize] as u16));
            }
        }
        (sub_system, new_r)
    }

    /// `isConditionImplied`: whether the constraints in the system imply
    /// `row`; false where the negation overflows.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstraintSystem.cpp#L271>
    pub fn is_condition_implied(
        &self,
        row: Row,
    ) -> bool {
        // If all variable coefficients are 0, we have 'C >= 0'. If the
        // constant is >= 0, R is always true, regardless of the system.
        if is_constant_only(&row) {
            return constant(&row) >= 0;
        }

        // If there is no solution with the negation of R added to the system,
        // the condition must hold based on the existing constraints.
        let row = Self::negate(row);
        if row.is_empty() {
            return false;
        }

        let mut copy = self.clone();
        copy.add_row(&row, self.num_variables);
        !copy.may_have_solution()
    }

    /// `isConditionImpliedInSubSystem`: `is_condition_implied` over the rows
    /// that share a variable with `row`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstraintSystem.cpp#L291>
    pub fn is_condition_implied_in_sub_system(
        &self,
        row: &[Entry],
    ) -> bool {
        if row.is_empty() {
            return false;
        }
        // Queries with no variables are trivially decided without building any
        // component.
        if is_constant_only(row) {
            return constant(row) >= 0;
        }
        // A single query: build the component and solve it in place.
        let (sub, new_row) = self.get_sub_system(row);
        sub.is_condition_implied(new_row)
    }
}

#[cfg(test)]
#[path = "constraintsystem_tests.rs"]
mod tests;
