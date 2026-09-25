//! The BASIC ABIs (section 15). BASIC passes an argument by reference, as a
//! near pointer into DGROUP, and a parameter takes it as an adapter that
//! `abi.<basic>` declares: `Ref[T]` borrows the variable as `&mut T`;
//! `StringRef` views its module's `string_data` and `string_length` of the
//! string descriptor; `ArrayRef[T, N]` views `abi.basic`'s `array_data` and
//! `array_count` of the array descriptor. What differs between the BASICs
//! is in those modules, not here.
//!
//! A library for BASIC links without the Nib runtime, whose start-up and
//! heap BASIC's own replace: a statement that calls it is refused.

use super::*;
use crate::frontends::nib::syntax::{Adapter, Basic};

impl TypeRegistry {
    /// The parameter an adapter type `spec` makes, when it is one.
    pub(super) fn adapter_parameter(&mut self, spec: &TypeSpec, span: Span) -> Result<Option<SignatureParameter>, Diagnostic> {
        let Some((basic, adapter)) = Adapter::of(spec) else {
            return Ok(None);
        };
        let arguments = match spec {
            TypeSpec::Applied { args, .. } => args.as_slice(),
            _ => &[],
        };
        let refused = || Diagnostic::new(span, format!("{}.{} takes {}", basic.name(), adapter.name(), match adapter {
            Adapter::Ref => "one type: Ref[T]",
            Adapter::String => "no type arguments",
            Adapter::Array => "an element type and a rank: ArrayRef[T, N]",
        }));
        let (target, pointer) = match (adapter, arguments) {
            (Adapter::Ref, [TypeAnnotation::Value(target)]) => {
                let element = self.crossing_element(target, span)?;
                let target = match element {
                    ElementType::Scalar(type_name) => BindingType::Scalar(type_name),
                    ElementType::Struct(id) => BindingType::Struct(id),
                };
                (target, self.raw_pointer(element, "near", true))
            }
            (Adapter::String, []) => {
                let descriptor = self.declared_struct(&format!("{}.StringDescriptor", basic.module()), span)?;
                let element = ElementType::Scalar(TypeName::Char);
                (BindingType::Slice { element, rank: 1 }, self.raw_pointer(descriptor, "near", false))
            }
            (Adapter::Array, [TypeAnnotation::Value(element)] | [TypeAnnotation::Slice { element, .. }]) => {
                let rank = match &arguments[0] {
                    TypeAnnotation::Slice { rank, .. } => *rank,
                    _ => 1,
                };
                let element = self.crossing_element(element, span)?;
                let descriptor = self.declared_struct("abi.basic.ArrayDescriptor", span)?;
                (BindingType::Slice { element, rank }, self.raw_pointer(descriptor, "near", false))
            }
            _ => return Err(refused()),
        };
        Ok(Some(SignatureParameter::Adapter { basic, adapter, target, pointer }))
    }

    /// `spec`, when BASIC has it: a scalar or a represented struct.
    fn crossing_element(&mut self, spec: &TypeSpec, span: Span) -> Result<ElementType, Diagnostic> {
        let element = self.resolve_element(spec, span)?;
        let crosses = match element {
            ElementType::Scalar(type_name) => !ownership::needs_drop(type_name),
            ElementType::Struct(id) => self.represented.contains(&id),
        };
        if !crosses {
            return Err(Diagnostic::new(span, "BASIC holds only scalars and represented structs"));
        }
        Ok(element)
    }

    fn declared_struct(&self, name: &str, span: Span) -> Result<ElementType, Diagnostic> {
        self.structs
            .get(name)
            .map(|one| ElementType::Struct(one.id))
            .ok_or_else(|| Diagnostic::new(span, format!("{name} is not declared")))
    }
}

impl FunctionCompiler<'_> {
    /// How the callee sees an adapter parameter, passed as the near pointer `value`.
    pub(super) fn adapter_binding(
        &mut self,
        basic: Basic,
        adapter: Adapter,
        target: BindingType,
        pointer: TypeName,
        value: u32,
        span: Span,
    ) -> Result<Binding, Diagnostic> {
        let BindingType::Slice { element, rank } = target else {
            return Ok(Binding { type_: target, mutable: true, storage: Storage::Reference(value) });
        };
        let descriptor = self.hidden("descriptor");
        let binding = Binding { type_: BindingType::Scalar(pointer), mutable: false, storage: Storage::Parameter(value) };
        self.scopes.last_mut().expect("scope").insert(descriptor.clone(), binding);
        let (module, data, length) = match adapter {
            Adapter::String => (basic.module(), "string_data", "string_length"),
            _ => ("abi.basic".to_owned(), "array_data", "array_count"),
        };
        // `module.function(descriptor)`, and the dimension it asks about.
        let read = |this: &mut Self, function: &str, dimension: Option<u8>| -> Result<u32, Diagnostic> {
            let arguments = std::iter::once(Expr::Name(descriptor.clone(), span))
                .chain(dimension.map(|one| Expr::Integer(i64::from(one), span)))
                .collect();
            this.call_abi(&format!("{module}.{function}"), arguments, span)
        };
        let data = read(self, data, None)?;
        let dimensions = match adapter {
            Adapter::String => vec![read(self, length, None)?],
            _ => (0..rank).map(|axis| read(self, length, Some(axis))).collect::<Result<_, _>>()?,
        };
        let view = self.view_slot(element, rank);
        let mut capacity = hir::Operand::Value(dimensions[0]);
        for (axis, dimension) in dimensions.iter().enumerate() {
            let place = hir::Operand::IndirectPlace { base: view, offset: descriptor::dim(axis as u8), type_id: U16, inbounds: false };
            self.emit("store", Vec::new(), vec![place, hir::Operand::Value(*dimension)], None);
            if axis > 0 {
                let product = self.value(TypeName::U16);
                self.emit("mul", vec![product], vec![capacity, hir::Operand::Value(*dimension)], None);
                capacity = hir::Operand::Value(product);
            }
        }
        let place = hir::Operand::IndirectPlace { base: view, offset: descriptor::capacity(rank), type_id: U16, inbounds: false };
        self.emit("store", Vec::new(), vec![place, capacity], None);
        let data_type = self.types.pointer(element.id(), 0);
        let elements = self.value_type(data_type);
        self.emit("copy", vec![elements], vec![hir::Operand::Value(data)], None);
        let place = hir::Operand::IndirectPlace { base: view, offset: descriptor::size(rank), type_id: data_type, inbounds: false };
        self.emit("store", Vec::new(), vec![place, hir::Operand::Value(elements)], None);
        Ok(Binding { type_: target, mutable: true, storage: Storage::Slice(view) })
    }

    /// The BASIC a program with a BASIC export or extern is a library for.
    /// That program runs on BASIC's stack, in DGROUP, so SS is DS there and a
    /// near pointer reaches a local too.
    pub(super) fn host(&self) -> Option<Basic> {
        self.signatures.values().find_map(|one| one.abi.basic())
    }

    /// Calls the ABI module's `function`: its result, as a value.
    fn call_abi(&mut self, function: &str, arguments: Vec<Expr>, span: Span) -> Result<u32, Diagnostic> {
        let call = Expr::Call { name: function.to_owned(), type_arguments: Vec::new(), arguments, span };
        self.unsafe_depth += 1;
        let result = self.expression(&call, None);
        self.unsafe_depth -= 1;
        let result = result?;
        let type_id = type_id(result.type_name);
        Ok(self.materialized(required(result, span)?, type_id))
    }

    /// A BASIC string function's result: the view in the `$result` slot,
    /// copied where BASIC takes it by its module's `string_result`.
    pub(super) fn string_result(&mut self, span: Span) -> Result<hir::Operand, Diagnostic> {
        let basic = self.signature.abi.basic().expect("a BASIC function");
        let Storage::Slice(view) = self.binding(RESULT, span)?.storage else {
            unreachable!("a view result's slot")
        };
        let element = ElementType::Scalar(TypeName::Char);
        let data_type = self.types.pointer(element.id(), 0);
        let raw = self.types.raw_pointer(element, "far", true);
        let data = self.value_type(data_type);
        let place = hir::Operand::IndirectPlace { base: view, offset: descriptor::size(1), type_id: data_type, inbounds: false };
        self.emit("load", vec![data], vec![place], None);
        let pointer = self.value(raw);
        self.emit("copy", vec![pointer], vec![hir::Operand::Value(data)], None);
        let length = self.value(TypeName::U16);
        let place = hir::Operand::IndirectPlace { base: view, offset: descriptor::dim(0), type_id: U16, inbounds: false };
        self.emit("load", vec![length], vec![place], None);
        let mut arguments = Vec::new();
        for (value, type_name) in [(pointer, raw), (length, TypeName::U16)] {
            let name = self.hidden("string");
            let binding = Binding { type_: BindingType::Scalar(type_name), mutable: false, storage: Storage::Parameter(value) };
            self.scopes.last_mut().expect("scope").insert(name.clone(), binding);
            arguments.push(Expr::Name(name, span));
        }
        let result = self.call_abi(&format!("{}.string_result", basic.module()), arguments, span)?;
        Ok(hir::Operand::Value(result))
    }

    /// Refuses the Nib runtime routine the code since call `since` calls,
    /// in a library for BASIC.
    pub(super) fn refuse_runtime(&self, since: usize, span: Span) -> Result<(), Diagnostic> {
        let Some(basic) = self.host() else {
            return Ok(());
        };
        let called = self.calls[since..].iter().find_map(|call| {
            self.builtin_ids.iter().find(|(_, (id, _))| *id == call.callee).map(|(name, _)| *name)
        });
        match called {
            Some(routine) => Err(Diagnostic::new(
                span,
                format!(
                    "this calls the Nib runtime ({routine}), which a library for {} links without; \
                     BASIC owns start-up and the heap",
                    basic.name()
                ),
            )),
            None => Ok(()),
        }
    }

    /// A near pointer to a temporary a BASIC float function stores its
    /// result in, which it passes last.
    pub(super) fn result_pointer(&mut self, pointer: TypeName) -> hir::Operand {
        let result = self.types.raw_target(pointer).expect("a raw pointer");
        let ElementType::Scalar(result) = result else {
            unreachable!("a float result")
        };
        let place = self.place("$result", result, true);
        let address = self.value(pointer);
        self.emit("address", vec![address], vec![hir::Operand::Place(place)], None);
        hir::Operand::Value(address)
    }
}
