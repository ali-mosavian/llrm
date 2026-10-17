//! A system of linear constraints: a translation of LLVM's `ConstraintSystem`
//! (`llvm/include/llvm/Analysis/ConstraintSystem.h`, `llvm/lib/Analysis/
//! ConstraintSystem.cpp` at d1106deb71cc), the engine of `ConstraintElimination`.
//!
//! A row is `c >= v1 * c1 + ... + vn * cn`, stored as `{constant, c1, ..., cn}`
//! by variable id (id 0 is the constant). A condition is implied when the
//! system with its negation has no solution, which Fourier-Motzkin elimination
//! decides, over only the rows connected to the query's variables.
//!
//! DEVIATE: none of the logic; `Value *` keys are `Operand`s (a constant too can be a variable), `SmallVector`s are
//! `Vec`s, and overflow is `checked_*` where LLVM uses `AddOverflow` and its
//! kin.

use llrm_mir::module::Operand;
use llrm_support::hash::IndexMap;

/// `ConstraintSystem::Entry`.
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

/// `ConstraintSystem::RowTy`.
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

/// `isConstantOnly`.
fn is_constant_only(row: &[Entry]) -> bool {
    row.is_empty() || (row.len() == 1 && row[0].id == 0)
}

/// `getConstant`.
fn constant(row: &[Entry]) -> i64 {
    if has_constant_entry(row) { row[0].coefficient } else { 0 }
}

/// Most rows a system may grow to while eliminating (`eliminateUsingFM`).
const MOST_ROWS: usize = 500;

#[derive(Clone, Debug, Default)]
pub struct ConstraintSystem {
    /// Variables not counting the constant part; they use ids 1 to this.
    num_variables: usize,
    constraints: Vec<Row>,
    /// A map of variables (IR values) to their index in the system.
    value2index: IndexMap<Operand, u32>,
}

impl ConstraintSystem {
    /// `ConstraintSystem(ArrayRef<Value *> FunctionArgs)`.
    pub fn with_arguments(arguments: &[Operand]) -> Self {
        let mut system = Self::default();
        system.num_variables += arguments.len();
        for &argument in arguments {
            let next = system.value2index.len() as u32 + 1;
            system.value2index.insert(argument, next);
        }
        system
    }

    /// `ConstraintSystem(const DenseMap<Value *, unsigned> &Value2Index)`.
    pub fn with_index(value2index: IndexMap<Operand, u32>) -> Self {
        Self { num_variables: value2index.len(), constraints: Vec::new(), value2index }
    }

    /// `addRow`: false where the row says nothing (only a constant).
    pub fn add_row(
        &mut self,
        row: &[Entry],
        num_vars: usize,
    ) -> bool {
        if is_constant_only(row) {
            return false;
        }
        debug_assert!(num_vars >= row.last().map_or(0, |last| last.id as usize), "NumVars must cover all variables in R");
        self.num_variables = num_vars.max(self.num_variables);
        // Only keep non-zero coefficients; in particular drop the entry for the
        // constant part if it is 0.
        self.constraints.push(row.iter().copied().filter(|entry| entry.coefficient != 0).collect());
        true
    }

    pub fn value2index(&self) -> &IndexMap<Operand, u32> {
        &self.value2index
    }

    pub fn value2index_mut(&mut self) -> &mut IndexMap<Operand, u32> {
        &mut self.value2index
    }

    /// `negate`: the negation of `R >= 0` is `-R + 1 >= 0`; empty on overflow.
    pub fn negate(mut row: Row) -> Row {
        debug_assert!(has_constant_entry(&row), "row must have a constant entry");
        match row[0].coefficient.checked_add(1) {
            Some(sum) => row[0].coefficient = sum,
            None => return Vec::new(),
        }
        Self::negate_or_equal(row)
    }

    /// `negateOrEqual`: every coefficient times -1; empty on overflow.
    pub fn negate_or_equal(mut row: Row) -> Row {
        for entry in &mut row {
            match entry.coefficient.checked_mul(-1) {
                Some(product) => entry.coefficient = product,
                None => return Vec::new(),
            }
        }
        row
    }

    /// `toStrictLessThan`: subtract 1 from the constant; empty on overflow.
    pub fn to_strict_less_than(mut row: Row) -> Row {
        debug_assert!(has_constant_entry(&row), "row must have a constant entry");
        match row[0].coefficient.checked_sub(1) {
            Some(difference) => row[0].coefficient = difference,
            None => return Vec::new(),
        }
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
        debug_assert!(self.num_variables >= n);
        self.num_variables -= n;
    }

    /// The number of rows.
    pub fn size(&self) -> usize {
        self.constraints.len()
    }

    /// `eliminateUsingFM`: Fourier-Motzkin elimination of the last variable,
    /// with the tricks of Pugh's Omega test; false where the system got too
    /// big to go on.
    fn eliminate_using_fm(&mut self) -> bool {
        debug_assert!(!self.constraints.is_empty(), "should only be called for non-empty constraint systems");
        let last_idx = self.num_variables as u16;

        // First, either remove the variable in place if it is 0 or add the row
        // to RemainingRows and remove it from the system.
        let mut remaining: Vec<Row> = Vec::new();
        let mut r1 = 0;
        while r1 < self.constraints.len() {
            if last_coefficient(&self.constraints[r1], last_idx) == 0 {
                let row = &mut self.constraints[r1];
                if !row.is_empty() && row.last().is_some_and(|last| last.id == last_idx) {
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
            for r2 in (r1 + 1)..count {
                let mut upper_last = last_coefficient(&remaining[r2], last_idx);
                let mut lower_last = last_coefficient(&remaining[r1], last_idx);
                debug_assert!(upper_last != 0 && lower_last != 0);

                if (lower_last < 0 && upper_last < 0) || (lower_last > 0 && upper_last > 0) {
                    continue;
                }

                let (mut lower_r, mut upper_r) = (r1, r2);
                if upper_last < 0 {
                    std::mem::swap(&mut lower_r, &mut upper_r);
                    std::mem::swap(&mut lower_last, &mut upper_last);
                }

                let mut new_row: Row = Vec::new();
                let (mut idx_upper, mut idx_lower) = (0usize, 0usize);
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
                    let (mut upper_v, mut lower_v) = (0i64, 0i64);
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

                    let Some(m1) = upper_v.checked_mul(lower_last.wrapping_neg()) else {
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
                    // elimination: the new coefficient for CurrentId is
                    // N = UpperV * (-1) * LowerLast + LowerV * UpperLast.
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
                if self.constraints.len() > MOST_ROWS {
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
        self.constraints.iter().all(|row| constant(row) >= 0)
    }

    /// `mayHaveSolution`: true if there may be a solution for the constraints.
    pub fn may_have_solution(&mut self) -> bool {
        self.may_have_solution_impl()
    }

    /// `getSubSystem`: the sub-system of constraints connected (transitively)
    /// to query `R`, with variables compacted to a dense index range, and `R`
    /// translated to it.
    pub fn sub_system(
        &self,
        query: &[Entry],
    ) -> (ConstraintSystem, Row) {
        debug_assert!(query.last().is_none_or(|last| last.id as usize <= self.num_variables), "query must only use variables of the system");
        // Only constraints that share a variable (transitively) with a query R
        // can affect whether system + !R has a solution. Mark variables in the
        // query and collect the transitive closure over variables that co-occur
        // in a constraint row.
        let mut sub = ConstraintSystem::default();
        let mut in_system = vec![false; self.num_variables + 1];
        for entry in query {
            if entry.id != 0 {
                in_system[entry.id as usize] = true;
            }
        }
        let shares = |in_system: &[bool], row: &[Entry]| row.iter().any(|entry| entry.id != 0 && in_system[entry.id as usize]);
        let mut changed = true;
        while changed {
            changed = false;
            for row in &self.constraints {
                // No common variables, skip.
                if !shares(&in_system, row) {
                    continue;
                }
                for entry in row {
                    if entry.id != 0 && !in_system[entry.id as usize] {
                        in_system[entry.id as usize] = true;
                        changed = true;
                    }
                }
            }
        }

        // Assign compact indices to the variables of the sub-system.
        let mut old_to_new = vec![0u32; self.num_variables + 1];
        let mut next = 1u32;
        for id in 0..in_system.len() {
            if in_system[id] {
                old_to_new[id] = next;
                next += 1;
            }
        }

        // Build new compact set of rows.
        sub.num_variables = (next - 1) as usize;
        for row in &self.constraints {
            if !shares(&in_system, row) {
                continue;
            }
            let new_row: Row = row
                .iter()
                .map(|entry| {
                    let new = old_to_new[entry.id as usize];
                    debug_assert!((entry.id == 0) == (new == 0), "constant entry must be preserved");
                    Entry::new(entry.coefficient, new as u16)
                })
                .collect();
            sub.constraints.push(new_row);
        }

        // Remap the query row into the component's compact index space.
        let mut new_query: Row = vec![Entry::new(constant(query), 0)];
        for entry in query {
            if entry.id != 0 {
                new_query.push(Entry::new(entry.coefficient, old_to_new[entry.id as usize] as u16));
            }
        }
        (sub, new_query)
    }

    /// `isConditionImplied`.
    pub fn is_condition_implied(
        &self,
        row: Row,
    ) -> bool {
        // If all variable coefficients are 0, we have 'C >= 0'. If the constant
        // is >= 0, R is always true, regardless of the system.
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
            return constant(row) >= 0;
        }

        // A single query: build the component and solve it in place.
        let (sub, new_row) = self.sub_system(row);
        sub.is_condition_implied(new_row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(entries: &[(i64, u16)]) -> Row {
        entries.iter().map(|&(coefficient, id)| Entry::new(coefficient, id)).collect()
    }

    /// `x <= 5` and `y <= x - 1` imply `y <= 4`; LLVM's own unit test shape
    /// (`ConstraintSystemTest.cpp`): rows are `c >= sum`, here `5 >= x` and
    /// `-1 >= y - x`.
    #[test]
    fn test_two_rows_imply_a_bound_on_the_second_variable() {
        let mut system = ConstraintSystem::default();
        assert!(system.add_row(&row(&[(5, 0), (1, 1)]), 2));
        assert!(system.add_row(&row(&[(-1, 0), (-1, 1), (1, 2)]), 2));
        // y <= 4: 4 >= y
        assert!(system.is_condition_implied(row(&[(4, 0), (1, 2)])));
        // y <= 3 is not implied
        assert!(!system.is_condition_implied(row(&[(3, 0), (1, 2)])));
    }

    /// A query shares no variable with the other rows: the sub-system is empty
    /// and the answer is the answer of the query alone.
    #[test]
    fn test_unrelated_rows_are_left_out_of_the_sub_system() {
        let mut system = ConstraintSystem::default();
        system.add_row(&row(&[(5, 0), (1, 1)]), 3);
        system.add_row(&row(&[(7, 0), (1, 3)]), 3);
        let (sub, query) = system.sub_system(&row(&[(4, 0), (1, 2)]));
        assert_eq!(sub.size(), 0);
        assert_eq!(query, row(&[(4, 0), (1, 1)]));
    }
}
