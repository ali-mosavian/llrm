//! Foreign interoperability: functions `@extern` imports, functions
//! `@export` exposes through a foreign ABI, and the `unsafe` blocks that call
//! them. Nib functions already follow cdecl16
//! -- far calls, arguments pushed right to left, the caller cleaning up --
//! so a foreign call differs only in its symbol and what may cross it.
//! An `interrupt16` function is not called at all: its far address is a
//! value, which the program installs as an interrupt vector.

use super::*;
use crate::syntax::{Extern, FOREIGN};

/// Whether a value of `type_name` may cross a foreign ABI: a scalar, or a
/// raw pointer to one or to a represented struct; never a buffer's owner.
fn crosses(types: &TypeRegistry, type_name: TypeName) -> bool {
    match type_name {
        TypeName::Pointer { type_id, .. } => {
            let target = types.types[(type_id - 1) as usize]
                .element
                .expect("a pointer has a target");
            types.types[(target - 1) as usize].kind != "opaque"
                || types.represented.contains(&target)
        }
        TypeName::String | TypeName::Vector { .. } => false,
        other => !ownership::needs_drop(other),
    }
}

/// Checks that `signature`, of the function `name`, may cross its foreign
/// ABI; a BASIC float result gets its hidden pointer.
pub(super) fn check_foreign(
    types: &mut TypeRegistry,
    signature: &mut Signature,
    name: &str,
    span: Span,
) -> Result<(), Diagnostic> {
    let basic = signature.abi.basic();
    interrupt_shape(signature.abi, signature.parameters.is_empty() && signature.returned(types) == TypeName::Void, span)?;
    for (parameter, (formal, _)) in signature.parameters.iter().zip(&signature.formals) {
        match parameter {
            SignatureParameter::Scalar(type_name) if crosses(types, *type_name) => {}
            SignatureParameter::Adapter { basic: own, .. } if Some(*own) == basic => {}
            SignatureParameter::Adapter { basic: own, adapter, .. } => return Err(misplaced(name, *own, *adapter, span)),
            _ => {
                return Err(Diagnostic::new(
                    span,
                    format!(
                        "{name}'s parameter {formal:?} cannot cross a foreign ABI; pass a scalar or a *far pointer"
                    ),
                ));
            }
        }
    }
    if let Some(basic) = basic {
        return basic_result(types, signature, basic, name, span);
    }
    // A represented struct of 4 bytes or less comes back in registers, as C's does.
    let result_crosses = match signature.slot {
        Some(struct_id) => signature.in_registers(types).is_some() && types.represented.contains(&struct_id),
        None => signature.view.is_none() && crosses(types, signature.result),
    };
    if !result_crosses {
        return Err(Diagnostic::new(
            span,
            format!("{name}'s result cannot cross a foreign ABI; return a scalar or a small represented struct"),
        ));
    }
    Ok(())
}

/// BASIC reads an INTEGER from `ax` and a LONG from `dx:ax`, and gives a
/// SINGLE or DOUBLE function a near pointer to store it through, pushed
/// last, which it returns in `ax`.
fn basic_result(
    types: &mut TypeRegistry,
    signature: &mut Signature,
    basic: super::super::syntax::Basic,
    name: &str,
    span: Span,
) -> Result<(), Diagnostic> {
    let refused = |what: &str| Diagnostic::new(span, format!("{name}'s result cannot cross to {}: {what}", basic.name()));
    const RESULTS: &str = "return an INTEGER, LONG, SINGLE, DOUBLE or a &string";
    // A string result is a view BASIC copies before the function returns.
    if signature.view == Some((ElementType::Scalar(TypeName::Char), 1)) && !signature.foreign {
        let descriptor = format!("{}.StringDescriptor", basic.module());
        let Some(layout) = types.structs.get(&descriptor) else {
            return Err(refused(&format!("import {} to return a string", basic.module())));
        };
        let descriptor = ElementType::Struct(layout.id);
        signature.string_result = Some(types.raw_pointer(descriptor, "near", false));
        return Ok(());
    }
    if signature.slot.is_some() || signature.view.is_some() {
        return Err(refused(RESULTS));
    }
    match signature.result {
        TypeName::Void | TypeName::I16 | TypeName::U16 | TypeName::I32 | TypeName::U32 => Ok(()),
        result @ (TypeName::F32 | TypeName::F64) => {
            signature.result_pointer = Some(types.raw_pointer(ElementType::Scalar(result), "near", true));
            Ok(())
        }
        _ => Err(refused(RESULTS)),
    }
}

fn misplaced(name: &str, basic: super::super::syntax::Basic, adapter: super::super::syntax::Adapter, span: Span) -> Diagnostic {
    Diagnostic::new(
        span,
        format!("{name} takes a {}.{}, which only a {} export or extern takes", basic.name(), adapter.name(), basic.name()),
    )
}

/// Refuses a BASIC adapter in a function no BASIC calls.
pub(super) fn check_adapters(signature: &Signature, name: &str, span: Span) -> Result<(), Diagnostic> {
    for parameter in &signature.parameters {
        if let SignatureParameter::Adapter { basic, adapter, .. } = parameter {
            return Err(misplaced(name, *basic, *adapter, span));
        }
    }
    Ok(())
}

/// The signature an `extern` function is called by: its object symbol, defined elsewhere.
pub(super) fn foreign_signature(
    types: &mut TypeRegistry,
    declared: &Extern,
    id: u32,
) -> Result<Signature, Diagnostic> {
    let mut signature = signature(types, &declared.function, id)?;
    signature.foreign = true;
    signature.abi = declared.abi;
    check_foreign(
        types,
        &mut signature,
        &declared.function.name,
        declared.function.span,
    )?;
    signature.name = declared.symbol.clone();
    Ok(signature)
}

/// An interrupt passes nothing and takes nothing back.
fn interrupt_shape(abi: Abi, takes_nothing_returns_void: bool, span: Span) -> Result<(), Diagnostic> {
    if abi.interrupt() && !takes_nothing_returns_void {
        return Err(Diagnostic::new(span, "an interrupt16 function takes nothing and returns void"));
    }
    Ok(())
}

impl TypeRegistry {
    /// `extern "abi" fn(A) -> R`: the far address of a function of that ABI
    /// whose type is `function`. Nothing reads or calls through it here; a
    /// foreign function does.
    pub(super) fn foreign_function(&mut self, abi: Abi, function: TypeName, span: Span) -> Result<TypeName, Diagnostic> {
        interrupt_shape(abi, self.types[(type_id(function) - 1) as usize].name == "fn() -> void", span)?;
        if let Some(found) = self.foreign_function_of(abi, function) {
            return Ok(found);
        }
        let name = self.foreign_name(abi, function);
        let id = self.pointer_type(name.clone(), type_id(function), 0, true);
        self.foreign_functions.insert(name, id);
        Ok(TypeName::Pointer { type_id: id, width: 4, mutable: false })
    }

    /// `extern "abi" fn(A) -> R`, when it is registered.
    pub(super) fn foreign_function_of(&self, abi: Abi, function: TypeName) -> Option<TypeName> {
        let type_id = *self.foreign_functions.get(&self.foreign_name(abi, function))?;
        Some(TypeName::Pointer { type_id, width: 4, mutable: false })
    }

    fn foreign_name(&self, abi: Abi, function: TypeName) -> String {
        format!("{FOREIGN} \"{}\" {}", abi.name(), self.types[(type_id(function) - 1) as usize].name)
    }
}

impl FunctionCompiler<'_> {
    /// The far address of `signature`'s function, typed by its ABI: data
    /// the linker writes, since only it knows where the code lands.
    pub(super) fn foreign_address(&mut self, signature: &Signature, expected: Option<TypeName>, span: Span) -> Result<TypedOperand, Diagnostic> {
        let function = self.types.function_type(signature);
        let type_name = self.types.foreign_function(signature.abi, function, span)?;
        if let Some(wanted) = expected.filter(|one| *one != type_name) {
            return Err(type_mismatch(span, wanted, type_name));
        }
        let symbol = self.literals.address(signature.id, &signature.name);
        let place = self.next_place;
        self.next_place += 1;
        self.places.push(hir::Place {
            id: place,
            name: format!("$address_{}", signature.name),
            type_id: type_id(type_name),
            mutable: false,
            offset: 0,
            extent: 4,
            storage: "module",
            symbol,
            volatile: false,
        });
        let value = self.value(type_name);
        self.emit("load", vec![value], vec![hir::Operand::Place(place)], None);
        Ok(TypedOperand { operand: Some(hir::Operand::Value(value)), type_name })
    }

    /// `unsafe: body`.
    pub(super) fn unsafe_block(&mut self, body: &[Statement]) -> Result<(), Diagnostic> {
        self.unsafe_depth += 1;
        let result = self.scoped(body);
        self.unsafe_depth -= 1;
        result
    }

    pub(super) fn require_unsafe(&self, what: &str, span: Span) -> Result<(), Diagnostic> {
        if self.unsafe_depth == 0 {
            return Err(Diagnostic::new(
                span,
                format!("{what} is unsafe: put it in an 'unsafe:' block"),
            ));
        }
        Ok(())
    }
}
