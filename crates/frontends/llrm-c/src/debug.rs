//! `-g`: C's side of the debug information -- the types and names the
//! front end described under -d2, mapped onto `llrm_hir::debug`, which
//! builds the rest.

use std::collections::HashMap;

use llrm_core::hir::debug::Builder;
use llrm_core::hir::model::{Debug, DebugKind, DebugReach, DebugScalar};

use super::hir::{self, DebugType};
use super::raise_hir::{signed, widths};

/// The unit's debug types as the builder's, each made once.
pub struct Described<'u> {
    unit: &'u hir::Unit,
    debug: &'u hir::Debug,
    builder: Builder,
    made: HashMap<i64, Option<i64>>,
}

impl<'u> Described<'u> {
    /// None unless the unit was compiled with -d2.
    pub fn of(unit: &'u hir::Unit) -> Option<Self> {
        Some(Self { unit, debug: unit.debug.as_ref()?, builder: Builder::for_language(llrm_core::hir::model::DebugLanguage::C, llrm_core::hir::model::DebugDialect::Cv4), made: HashMap::new() })
    }

    /// A pointer's reach, as the memory model makes a default one.
    fn reach(&self, cg: &str) -> Option<DebugReach> {
        Some(match cg {
            "TY_NEAR_POINTER" | "TY_NEAR_CODE_PTR" => DebugReach::Near,
            "TY_LONG_POINTER" | "TY_LONG_CODE_PTR" => DebugReach::Far,
            "TY_HUGE_POINTER" => DebugReach::Huge,
            "TY_POINTER" => DebugReach::Near,
            "TY_CODE_PTR" if self.unit.target & hir::BIG_CODE == 0 => DebugReach::Near,
            "TY_POINTER" | "TY_CODE_PTR" => DebugReach::Far,
            _ => return None,
        })
    }

    /// C's scalar `name` of the code generator's type `cg`, as wide and
    /// signed as the code keeps it.
    fn scalar(name: &str, cg: &str) -> Option<DebugScalar> {
        if cg == "TY_DEFAULT" && name == "void" {
            return Some(DebugScalar::Void);
        }
        let float = matches!(cg, "TY_SINGLE" | "TY_DOUBLE" | "TY_LONG_DOUBLE");
        Some(match (float, widths(cg)?, signed(cg)) {
            (true, 4, _) => DebugScalar::Float32,
            (true, 8, _) => DebugScalar::Float64,
            (true, 10, _) => DebugScalar::Float80,
            (false, 1, true) if name == "char" => DebugScalar::Char,
            (false, 1, true) => DebugScalar::Int8,
            (false, 1, false) => DebugScalar::UInt8,
            (false, 2, true) => DebugScalar::Int16,
            (false, 2, false) => DebugScalar::UInt16,
            (false, 4, true) => DebugScalar::Int32,
            (false, 4, false) => DebugScalar::UInt32,
            (false, 8, true) => DebugScalar::Int64,
            (false, 8, false) => DebugScalar::UInt64,
            _ => return None,
        })
    }

    /// The bytes a value of `handle` takes.
    fn size(&self, handle: i64) -> Option<i64> {
        match self.debug.types.get(&handle)? {
            DebugType::Scalar { cg, .. } | DebugType::Enum { cg } => widths(cg).map(i64::from),
            DebugType::Pointer { cg, .. } => match self.reach(cg)? {
                DebugReach::Near => Some(2),
                DebugReach::Far | DebugReach::Huge => Some(4),
            },
            DebugType::Array { hi, base } => Some((hi + 1) * self.size(*base)?),
            DebugType::Struct { size, .. } => Some(*size),
            DebugType::Name { target, .. } => self.size((*target)?),
            DebugType::Proc { .. } => None,
        }
    }

    /// `handle`'s type, where CodeView has one for it.
    fn r#type(&mut self, handle: i64) -> Option<i64> {
        if let Some(&made) = self.made.get(&handle) {
            return made;
        }
        let made = match self.debug.types.get(&handle)?.clone() {
            DebugType::Scalar { name, cg } => Self::scalar(&name, &cg).map(|one| match one {
                DebugScalar::Void => self.builder.scalar(one),
                // The spelling is the source's own, which a debugger prints.
                _ => self.builder.spelled_scalar(one, &name),
            }),
            DebugType::Enum { cg } => Self::scalar("", &cg).map(|one| self.builder.scalar(one)),
            DebugType::Pointer { cg, base } => {
                let reach = self.reach(&cg);
                let target = self.r#type(base).or_else(|| Some(self.builder.scalar(DebugScalar::Void)));
                reach.zip(target).map(|(reach, target)| self.builder.pointer(target, reach))
            }
            DebugType::Array { base, .. } => {
                let (element, bytes) = (self.r#type(base), self.size(handle));
                element.zip(bytes).map(|(element, bytes)| self.builder.sized(element, bytes))
            }
            // An aggregate is declared before its members, which may point back to it: the only
            // place a type reaches itself, so the only one that needs this.
            DebugType::Struct { name, union, size, fields } => {
                let id = self.builder.declare_aggregate(if union { DebugKind::Union } else { DebugKind::Struct }, &name, size);
                self.made.insert(handle, Some(id));
                let mut members = Vec::new();
                for (offset, field, handle, bits) in &fields {
                    if let Some(r#type) = self.r#type(*handle) {
                        members.push((field.clone(), r#type, *offset, *bits));
                    }
                }
                let members: Vec<(&str, i64, i64, Option<(i64, i64)>)> = members.iter().map(|(name, r#type, offset, bits)| (name.as_str(), *r#type, *offset, *bits)).collect();
                self.builder.define_aggregate(id, &members);
                Some(id)
            }
            DebugType::Proc { result, parameters } => {
                let void = self.builder.scalar(DebugScalar::Void);
                let result = self.r#type(result).filter(|&one| one != void);
                // `(void)`: no parameter.
                let parameters: Option<Vec<i64>> = parameters.iter().map(|&one| self.r#type(one)).filter(|&one| one != Some(void)).collect();
                parameters.map(|parameters| self.builder.procedure(result, &parameters))
            }
            DebugType::Name { target, .. } => target.and_then(|target| self.r#type(target)),
        };
        self.made.insert(handle, made);
        made
    }

    /// The module's variable `name` in data object `object`.
    pub fn global(&mut self, object: i64, name: &str, handle: i64) {
        if let Some(r#type) = self.r#type(handle) {
            self.builder.global(object, 0, name, r#type);
        }
    }

    /// A parameter's or local's `name`, held in `place`.
    pub fn variable(&mut self, place: i64, name: &str, handle: i64, parameter: bool) {
        if let Some(r#type) = self.r#type(handle) {
            self.builder.variable(place, name, r#type, parameter);
        }
    }

    /// A parameter's home, `place`, that holds the function's `argument`th argument once it has stored it.
    pub fn parameter_home(&mut self, place: i64, name: &str, handle: i64, argument: i64) {
        if let Some(r#type) = self.r#type(handle) {
            self.builder.parameter_home(place, name, r#type, argument);
        }
    }

    /// A function's static `name`, in data object `object`.
    pub fn local_static(&mut self, object: i64, name: &str, handle: i64) {
        if let Some(r#type) = self.r#type(handle) {
            self.builder.local_static(object, 0, name, r#type);
        }
    }

    /// The function `function` just compiled, `name`, of the procedure type
    /// `handle`.
    pub fn function(&mut self, function: i64, name: &str, handle: Option<i64>) {
        let r#type = handle.and_then(|handle| self.r#type(handle)).unwrap_or_else(|| self.builder.procedure(None, &[]));
        self.builder.typed_function(function, name, r#type);
    }

    /// The unit's globals, each by the data object `object` gives its symbol.
    pub fn finish(mut self, object: impl Fn(i64) -> Option<i64>) -> Debug {
        for &(symbol, handle) in &self.debug.globals.clone() {
            let (Some(at), Some(one)) = (object(symbol), self.unit.symbols.get(&symbol)) else { continue };
            let name = one.name.clone();
            self.global(at, &name, handle);
        }
        self.builder.finish()
    }
}
