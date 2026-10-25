//! `ConstraintSystem`: a system of linear constraints over integer variables,
//! decided by Fourier-Motzkin elimination
//! ([ConstraintSystem.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstraintSystem.cpp),
//! [ConstraintSystem.h](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ConstraintSystem.h)).
//! CE-1 of docs/working/constraint-elimination-plan.md.
//!
//! # How it works
//! A row is `c >= v1*c1 + ... + vn*cn`, held sparse as `(coefficient, id)`
//! entries by ascending id; id 0 is the constant. [`ConstraintSystem::
//! may_have_solution`] eliminates the highest variable at a time: the rows that
//! do not hold it stay, and each pair with the variable's coefficient of
//! opposite signs combines into one without it. A coefficient that would
//! overflow `i64` drops that combination, and a system that grows past 500
//! rows stops the elimination, both answering "may have a solution".
//! [`ConstraintSystem::is_condition_implied`] adds the negation of a row and
//! asks whether any solution is left; the `InSubSystem` form first keeps only
//! the rows connected to the query's variables.
//!
//! # Deviations from LLVM
//! - D1: the variable map is generic in its key (LLVM's is `Value *`), so the
//!   system is tested without a function; `dump` is not translated.
//!
//! # Example
//! `{0, 1, -2}` (`0 >= x - 2y`) and `{3, 2, 3}` (`3 >= 2x + 3y`) eliminate `y`
//! to `{6, 7}`: `6 >= 7x`.

use std::collections::HashMap;
use std::hash::Hash;

/// `ConstraintSystem::Entry`: a coefficient and the variable it belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

/// `ConstraintSystem::RowTy`: one constraint of the form `c >= v1*c1 + ...`.
pub type Row = Vec<Entry>;

/// `getLastCoefficient`.
fn last_coefficient(
    row: &[Entry],
    id: u16,
) -> i64 {
    match row.last() {
        Some(last) if last.id == id => last.coefficient,
        _ => 0,
    }
}

/// `hasConstantEntry`.
fn has_constant_entry(row: &[Entry]) -> bool {
    row.first().is_some_and(|first| first.id == 0)
}

/// `isConstantOnly`: of the form `c >= 0`.
fn is_constant_only(row: &[Entry]) -> bool {
    row.is_empty() || (row.len() == 1 && row[0].id == 0)
}

/// `getConstant`: 0 if the row has no entry for it.
fn constant_of(row: &[Entry]) -> i64 {
    if has_constant_entry(row) { row[0].coefficient } else { 0 }
}

/// `ConstraintSystem`, over variables keyed by `K` (LLVM's `Value *`).
#[derive(Clone, Debug)]
pub struct ConstraintSystem<K: Copy + Eq + Hash = u32> {
    /// Number of variables, not counting the constant part: ids 1 to this.
    num_variables: usize,
    constraints: Vec<Row>,
    value2index: HashMap<K, u32>,
}

impl<K: Copy + Eq + Hash> Default for ConstraintSystem<K> {
    fn default() -> Self {
        Self { num_variables: 0, constraints: Vec::new(), value2index: HashMap::new() }
    }
}

impl<K: Copy + Eq + Hash> ConstraintSystem<K> {
    /// `ConstraintSystem(FunctionArgs)`: the arguments are variables 1..n.
    pub fn with_arguments(arguments: &[K]) -> Self {
        let mut system = Self { num_variables: arguments.len(), ..Self::default() };
        for &one in arguments {
            let next = system.value2index.len() as u32 + 1;
            system.value2index.entry(one).or_insert(next);
        }
        system
    }

    /// `ConstraintSystem(Value2Index)`.
    pub fn with_index(value2index: HashMap<K, u32>) -> Self {
        Self { num_variables: value2index.len(), constraints: Vec::new(), value2index }
    }

    /// `getValue2Index`.
    pub fn value2index(&self) -> &HashMap<K, u32> {
        &self.value2index
    }

    pub fn value2index_mut(&mut self) -> &mut HashMap<K, u32> {
        &mut self.value2index
    }

    /// `addRow`: false where the row says nothing about a variable.
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
        debug_assert!(num_vars >= row.last().expect("a row").id as usize, "NumVars must cover all variables in R");
        self.num_variables = num_vars.max(self.num_variables);
        // Only keep non-zero coefficients; in particular drop the entry for
        // the constant part if it is 0.
        self.constraints.push(row.iter().copied().filter(|one| one.coefficient != 0).collect());
        true
    }

    /// `negate`: the negation of `c >= ...`, empty on overflow.
    pub fn negate(mut row: Row) -> Row {
        debug_assert!(has_constant_entry(&row), "row must have a constant entry");
        // The negated constraint R is obtained by multiplying by -1 and adding
        // 1 to the constant.
        let Some(constant) = row[0].coefficient.checked_add(1) else { return Vec::new() };
        row[0].coefficient = constant;
        Self::negate_or_equal(row)
    }

    /// `negateOrEqual`: every coefficient times -1, empty on overflow.
    pub fn negate_or_equal(mut row: Row) -> Row {
        for one in &mut row {
            let Some(negated) = one.coefficient.checked_mul(-1) else { return Vec::new() };
            one.coefficient = negated;
        }
        row
    }

    /// `toStrictLessThan`: the constant less 1, empty on overflow.
    pub fn to_strict_less_than(mut row: Row) -> Row {
        debug_assert!(has_constant_entry(&row), "row must have a constant entry");
        // The strict less than is obtained by subtracting 1 from the constant.
        let Some(constant) = row[0].coefficient.checked_sub(1) else { return Vec::new() };
        row[0].coefficient = constant;
        row
    }

    /// `getLastConstraint`.
    pub fn last_constraint(&self) -> &Row {
        self.constraints.last().expect("Constraint system is empty")
    }

    /// `popLastConstraint`.
    pub fn pop_last_constraint(&mut self) {
        self.constraints.pop();
    }

    /// `popLastNVariables`.
    pub fn pop_last_n_variables(
        &mut self,
        n: usize,
    ) {
        debug_assert!(self.num_variables >= n);
        self.num_variables -= n;
    }

    /// `size`: the number of rows.
    pub fn size(&self) -> usize {
        self.constraints.len()
    }

    /// `eliminateUsingFM`: false where the system grew too big to go on.
    fn eliminate_using_fm(&mut self) -> bool {
        // Implementation of Fourier-Motzkin elimination, with some tricks from
        // Pugh, "The Omega test: a fast and practical integer programming
        // algorithm for dependence analysis" (Supercomputing '91).
        debug_assert!(!self.constraints.is_empty(), "should only be called for non-empty constraint systems");
        let last_idx = self.num_variables as u16;

        // First, either remove the variable in place if it is 0 or add the row
        // to RemainingRows and remove it from the system.
        let mut remaining: Vec<Row> = Vec::new();
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
                remaining.push(self.constraints.pop().expect("a row"));
            }
        }

        // Process rows where the variable is != 0.
        let count = remaining.len();
        for r1 in 0..count {
            for r2 in r1 + 1..count {
                // Examples of constraints stored as {Constant, Coeff_x,
                // Coeff_y} R1:  0 >=  1 * x + (-2) * y  => { 0,
                // 1, -2 } R2:  3 >=  2 * x +  3 * y    => { 3,
                // 2,  3 } LastIdx = 2 (tracking coefficient of
                // y) UpperLast: 3
                // LowerLast: -2
                let mut upper_last = last_coefficient(&remaining[r2], last_idx);
                let mut lower_last = last_coefficient(&remaining[r1], last_idx);
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

                let mut new_row: Row = Vec::new();
                let (mut idx_upper, mut idx_lower) = (0, 0);
                let (lower_row, upper_row) = (&remaining[lower_r], &remaining[upper_r]);
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
                    let Some(m1) = upper_v.checked_mul(-lower_last) else {
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
                    // elimination. The new coefficient for CurrentId is
                    // N = UpperV * (-1) * LowerLast + LowerV * UpperLast
                    //
                    // UpperRow: { 3,  2,  3 }, LowerLast: -2
                    // LowerRow: { 0,  1, -2 }, UpperLast: 3
                    // After multiplication: { 6, 4, 6 } and { 0, 3, -6 }
                    // Eliminates y after addition: { 6, 7, 0 } => 6 >= 7 * x
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
        self.constraints.iter().all(|row| constant_of(row) >= 0)
    }

    /// `mayHaveSolution`: whether the constraints may have a solution. Works
    /// on the system, which it leaves with the variables eliminated.
    pub fn may_have_solution(&mut self) -> bool {
        self.may_have_solution_impl()
    }

    /// `getSubSystem`: the rows connected (transitively) to the query `row`,
    /// with variables compacted to a dense index range, and `row` translated
    /// into it.
    pub fn sub_system(
        &self,
        row: &[Entry],
    ) -> (ConstraintSystem<K>, Row) {
        debug_assert!(
            row.last().is_none_or(|last| last.id as usize <= self.num_variables),
            "query must only use variables of the system"
        );
        // Only constraints that share a variable (transitively) with a query R
        // can affect whether system + !R has a solution. Mark variables in the
        // query and collect to the transitive closure over variables that
        // co-occur in a constraint row.
        let mut sub = ConstraintSystem::default();
        let mut in_system = vec![false; self.num_variables + 1];
        for one in row {
            if one.id != 0 {
                in_system[one.id as usize] = true;
            }
        }
        let shares =
            |row: &[Entry], in_system: &[bool]| row.iter().any(|one| one.id != 0 && in_system[one.id as usize]);
        let mut changed = true;
        while changed {
            changed = false;
            for each in &self.constraints {
                // No common variables, skip.
                if !shares(each, &in_system) {
                    continue;
                }
                for one in each {
                    if one.id != 0 && !in_system[one.id as usize] {
                        in_system[one.id as usize] = true;
                        changed = true;
                    }
                }
            }
        }

        // Assign compact indices to the variables of the sub-system.
        let mut old_to_new = vec![0_u32; self.num_variables + 1];
        let mut next = 1_u32;
        for (id, &set) in in_system.iter().enumerate() {
            if set {
                old_to_new[id] = next;
                next += 1;
            }
        }

        // Build new compact set of rows.
        sub.num_variables = (next - 1) as usize;
        for each in &self.constraints {
            if !shares(each, &in_system) {
                continue;
            }
            let new_row = each
                .iter()
                .map(|one| {
                    let new = old_to_new[one.id as usize];
                    debug_assert!((one.id == 0) == (new == 0), "constant entry must be preserved");
                    Entry::new(one.coefficient, new as u16)
                })
                .collect();
            sub.constraints.push(new_row);
        }

        // Remap the query row into the component's compact index space.
        let mut new_r = vec![Entry::new(constant_of(row), 0)];
        for one in row {
            if one.id != 0 {
                new_r.push(Entry::new(one.coefficient, old_to_new[one.id as usize] as u16));
            }
        }
        (sub, new_r)
    }

    /// `isConditionImplied`: whether `row` holds in every solution.
    pub fn is_condition_implied(
        &self,
        row: Row,
    ) -> bool {
        // If all variable coefficients are 0, we have 'C >= 0'. If the
        // constant is >= 0, R is always true, regardless of the system.
        if is_constant_only(&row) {
            return constant_of(&row) >= 0;
        }
        // If there is no solution with the negation of R added to the system,
        // the condition must hold based on the existing constraints.
        let negated = Self::negate(row);
        if negated.is_empty() {
            return false;
        }
        let mut copy = self.clone();
        copy.add_row(&negated, self.num_variables);
        !copy.may_have_solution()
    }

    /// `isConditionImpliedInSubSystem`.
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
            return constant_of(row) >= 0;
        }
        // A single query: build the component and solve it in place.
        let (sub, new_r) = self.sub_system(row);
        sub.is_condition_implied(new_r)
    }
}

#[cfg(test)]
#[path = "constraintsystem_tests.rs"]
mod tests;
