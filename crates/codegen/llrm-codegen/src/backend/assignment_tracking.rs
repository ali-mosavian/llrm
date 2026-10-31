//! `AssignmentTrackingAnalysis`: where each variable of a function lives, from
//! its `#dbg_assign` records and the stores they name, as a table of locations
//! the code generator emits.
//!
//! # How it works
//! This file holds the result, [`FunctionVarLocs`], and the table it is built
//! in, [`FunctionVarLocsBuilder`]: the variables (a variable and the bit range
//! of it a location covers), the variables that have one location for their
//! whole scope, and the locations that start just before an instruction. The
//! analysis that fills it comes in later commits.
//!
//! # Example
//! A variable `x` kept in `%a` from its first use, and `y` in its stack slot
//! for the whole function:
//!
//! ```text
//! === Variables ===
//! [1] x
//! [2] y
//! === Single location vars ===
//! DEF Var=[2] Expr=!DIExpression(DW_OP_deref) Values=(%slot )
//! === In-line variable defs ===
//! DEF Var=[1] Expr=!DIExpression() Values=(%a )
//! ```
//!
//! # Complexity
//! Linear in the locations: a variable is found by a hash of its key, a wedge
//! by its instruction. The analysis that builds it keeps the pin's bound (a
//! fixpoint over blocks and variables, cut off at `MAX_BLOCKS`).
//!
//! # See also
//! `llvm/lib/CodeGen/AssignmentTrackingAnalysis.cpp`, pin d1106deb71cc.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

use llrm_mir::{DwOp, InstId, MetadataId, Module, Operand};

/// A number of a variable in [`FunctionVarLocs::variables`]; 0 is reserved
/// (`VariableID::Reserved`), as the pin's table is one-based.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VariableId(pub u32);

/// The bits of a variable a location covers (`DIExpression::FragmentInfo`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Fragment {
    pub offset: u64,
    pub size: u64,
}

/// A variable, or a fragment of it, as one inlined instance of it
/// (`DebugVariable`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DebugVariable {
    /// The variable's node (`DILocalVariable`).
    pub variable: MetadataId,
    pub fragment: Option<Fragment>,
    /// The `DILocation` of the call this instance was inlined at.
    pub inlined_at: Option<MetadataId>,
}

/// One definition: the variable is what the expression makes of the values,
/// from this point (`VarLocInfo`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VarLocInfo {
    pub variable: VariableId,
    pub expression: Vec<DwOp>,
    pub location: Option<MetadataId>,
    /// The values the expression reads; none for a location nothing says.
    pub values: Vec<Operand>,
}

/// The result: the locations of one function (`FunctionVarLocs`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FunctionVarLocs {
    variables: Vec<DebugVariable>,
    single: Vec<VarLocInfo>,
    before: BTreeMap<InstId, Vec<VarLocInfo>>,
}

/// What [`FunctionVarLocs`] is built in (`FunctionVarLocsBuilder`).
#[derive(Debug, Default)]
pub struct FunctionVarLocsBuilder {
    variables: Vec<DebugVariable>,
    ids: HashMap<DebugVariable, VariableId>,
    single: Vec<VarLocInfo>,
    before: BTreeMap<InstId, Vec<VarLocInfo>>,
}

impl FunctionVarLocsBuilder {
    pub fn number_of_variables(&self) -> usize {
        self.variables.len()
    }

    /// The id of `variable`, new if it has none (`insertVariable`).
    pub fn insert_variable(
        &mut self,
        variable: DebugVariable,
    ) -> VariableId {
        if let Some(&id) = self.ids.get(&variable) {
            return id;
        }
        self.variables.push(variable);
        let id = VariableId(self.variables.len() as u32);
        self.ids.insert(variable, id);
        id
    }

    pub fn variable(
        &self,
        id: VariableId,
    ) -> &DebugVariable {
        &self.variables[id.0 as usize - 1]
    }

    /// The locations that start just before `inst`, none if there are none.
    pub fn wedge(
        &self,
        inst: InstId,
    ) -> Option<&[VarLocInfo]> {
        self.before.get(&inst).map(Vec::as_slice)
    }

    /// Replaces the locations that start just before `inst` (`setWedge`).
    pub fn set_wedge(
        &mut self,
        inst: InstId,
        wedge: Vec<VarLocInfo>,
    ) {
        self.before.insert(inst, wedge);
    }

    /// A location valid for the variable's whole scope (`addSingleLocVar`).
    pub fn add_single_loc_var(
        &mut self,
        variable: DebugVariable,
        expression: Vec<DwOp>,
        location: Option<MetadataId>,
        values: Vec<Operand>,
    ) {
        let variable = self.insert_variable(variable);
        self.single.push(VarLocInfo { variable, expression, location, values });
    }

    /// A location that starts just before `before` (`addVarLoc`).
    pub fn add_var_loc(
        &mut self,
        before: InstId,
        variable: DebugVariable,
        expression: Vec<DwOp>,
        location: Option<MetadataId>,
        values: Vec<Operand>,
    ) {
        let variable = self.insert_variable(variable);
        self.before.entry(before).or_default().push(VarLocInfo { variable, expression, location, values });
    }

    /// The result (`FunctionVarLocs::init`).
    pub fn finish(self) -> FunctionVarLocs {
        FunctionVarLocs { variables: self.variables, single: self.single, before: self.before }
    }
}

impl FunctionVarLocs {
    pub fn variable(
        &self,
        id: VariableId,
    ) -> &DebugVariable {
        &self.variables[id.0 as usize - 1]
    }

    /// The locations valid for a variable's whole scope.
    pub fn single_locs(&self) -> &[VarLocInfo] {
        &self.single
    }

    /// The locations that start just before `inst`.
    pub fn locs_before(
        &self,
        inst: InstId,
    ) -> &[VarLocInfo] {
        self.before.get(&inst).map_or(&[], Vec::as_slice)
    }

    /// The instructions with locations before them, in order.
    pub fn instructions(&self) -> impl Iterator<Item = InstId> + '_ {
        self.before.keys().copied()
    }

    /// `FunctionVarLocs::print` for the parts that do not need the function:
    /// the variable table, the single locations and the locations by
    /// instruction. `name` says a value and `variable` a variable's name.
    pub fn print(
        &self,
        module: &Module,
        variable_name: &dyn Fn(MetadataId) -> String,
        value_name: &dyn Fn(Operand) -> String,
    ) -> String {
        let mut out = String::from("=== Variables ===\n");
        for (at, one) in self.variables.iter().enumerate() {
            let _ = write!(out, "[{}] {}", at + 1, variable_name(one.variable));
            if let Some(fragment) = one.fragment {
                let _ = write!(out, " bits [{}, {})", fragment.offset, fragment.offset + fragment.size);
            }
            if let Some(inlined) = one.inlined_at {
                let _ = write!(out, " inlined-at !{}", inlined.0);
            }
            out.push('\n');
        }
        let line = |out: &mut String, one: &VarLocInfo| {
            let _ = write!(
                out,
                "DEF Var=[{}] Expr=!DIExpression({}) Values=(",
                one.variable.0,
                expression_text(&one.expression)
            );
            for value in &one.values {
                let _ = write!(out, "{} ", value_name(*value));
            }
            out.push_str(")\n");
        };
        let _ = module;
        out.push_str("=== Single location vars ===\n");
        for one in &self.single {
            line(&mut out, one);
        }
        out.push_str("=== In-line variable defs ===\n");
        for (inst, wedge) in &self.before {
            let _ = writeln!(out, "before %{}:", inst.0);
            for one in wedge {
                line(&mut out, one);
            }
        }
        out
    }
}

/// The operations of an expression as LLVM prints them.
pub fn expression_text(ops: &[DwOp]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for op in ops {
        parts.push(match op {
            DwOp::PlusUconst(n) => format!("DW_OP_plus_uconst, {n}"),
            DwOp::StackValue => "DW_OP_stack_value".to_owned(),
            DwOp::Fragment { offset, size } => format!("DW_OP_LLVM_fragment, {offset}, {size}"),
            DwOp::Arg(n) => format!("DW_OP_LLVM_arg, {n}"),
            DwOp::Constu(n) => format!("DW_OP_constu, {n}"),
            DwOp::Consts(n) => format!("DW_OP_consts, {n}"),
            DwOp::Deref => "DW_OP_deref".to_owned(),
            DwOp::Binary(_) | DwOp::Unary(_) | DwOp::Other => "DW_OP_?".to_owned(),
        });
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variable(node: u32) -> DebugVariable {
        DebugVariable { variable: MetadataId(node), fragment: None, inlined_at: None }
    }

    /// A variable has one id however often it is asked for, and the ids start
    /// at 1.
    #[test]
    fn a_variable_has_one_id_and_ids_start_at_one() {
        let mut builder = FunctionVarLocsBuilder::default();
        let x = builder.insert_variable(variable(5));
        let y = builder.insert_variable(variable(6));
        assert_eq!((x, y, builder.insert_variable(variable(5))), (VariableId(1), VariableId(2), VariableId(1)));
        let piece = DebugVariable { fragment: Some(Fragment { offset: 0, size: 8 }), ..variable(5) };
        assert_eq!(builder.insert_variable(piece), VariableId(3), "a fragment is another variable");
        assert_eq!(builder.number_of_variables(), 3);
    }

    /// Locations stand before their instruction in the order they were added,
    /// and single locations apart from them.
    #[test]
    fn locations_keep_their_instruction_and_order() {
        let mut builder = FunctionVarLocsBuilder::default();
        builder.add_var_loc(InstId(3), variable(5), vec![], None, vec![]);
        builder.add_var_loc(InstId(3), variable(6), vec![DwOp::Deref], None, vec![]);
        builder.add_single_loc_var(variable(7), vec![DwOp::Deref], None, vec![]);
        let result = builder.finish();
        let names: Vec<u32> = result.locs_before(InstId(3)).iter().map(|one| one.variable.0).collect();
        assert_eq!(names, [1, 2]);
        assert!(result.locs_before(InstId(4)).is_empty());
        assert_eq!(result.single_locs().len(), 1);
        assert_eq!(result.variable(VariableId(3)).variable, MetadataId(7));
    }
}
