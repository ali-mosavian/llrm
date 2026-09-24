//! User `drop` methods (section 8): `fn T.drop(self: &mut T)` runs once
//! when an owner of a T ends, before what T's fields own is dropped. A
//! moved-from T has no null to mark it, so each owning local of a type that
//! holds one carries a flag, cleared when its value moves.

use super::moves::{owner, Owner};
use super::*;

impl TypeRegistry {
    /// Records each struct's `drop` method, checking its form.
    pub(super) fn register_drops(&mut self, functions: &[&Function]) -> Result<(), Diagnostic> {
        for function in functions {
            let Some((type_name, "drop")) = function.name.rsplit_once('.') else {
                continue;
            };
            let Some(layout) = self.structs.get(type_name) else {
                continue;
            };
            let takes_self = matches!(
                function.parameters.as_slice(),
                [Parameter { type_: ParameterType::Borrowed { mutable: true, target: TypeAnnotation::Value(TypeSpec::Named(target)) }, .. }]
                    if target == type_name
            );
            if !takes_self || function.result != TypeAnnotation::Value(TypeSpec::Primitive(TypeName::Void)) {
                return Err(Diagnostic::new(function.span, format!("a drop method is 'fn {type_name}.drop(self: &mut {type_name}) -> void'")));
            }
            self.dropped.insert(layout.id, function.name.clone());
        }
        Ok(())
    }
}

impl FunctionCompiler<'_> {
    /// Whether a value of this type holds something with a `drop` method.
    pub(super) fn holds_user_drop(&self, element: ElementType) -> bool {
        match element {
            ElementType::Scalar(type_name) => {
                self.types.sequence_element(type_name).is_some_and(|inner| self.holds_user_drop(inner))
            }
            ElementType::Struct(id) => {
                self.types.dropped.contains_key(&id)
                    || self.types.structure(id).expect("registered layout").fields.values().any(|field| self.holds_user_drop(field.type_))
            }
        }
    }

    /// Refuses to duplicate what holds a resource.
    pub(super) fn check_copyable(&self, element: ElementType, span: Span) -> Result<(), Diagnostic> {
        if self.holds_user_drop(element) {
            return Err(Diagnostic::new(span, "a value holding a type with a drop method cannot be copied"));
        }
        Ok(())
    }

    /// Calls the `drop` method of `view`'s type, if it has one.
    pub(super) fn call_drop(&mut self, view: &StructView) {
        let Some(name) = self.types.dropped.get(&view.struct_id).cloned() else {
            return;
        };
        let callee = self.signatures[&name].id;
        let pointer = self.address_of(view);
        let instruction = self.emit("call", Vec::new(), vec![pointer], Some(name));
        self.calls.push(hir::CallSite::new(instruction, callee, 1, Abi::Cdecl16));
    }

    pub(super) fn is_drop_method(&self, name: &str) -> bool {
        self.types.dropped.values().any(|one| one == name)
    }

    /// Makes `storage` an owner of a `struct_id`, flagged live when it holds a `drop`.
    pub(super) fn own_aggregate(&mut self, storage: &Storage, struct_id: u32) {
        match storage {
            Storage::Place(place) => self.own(*place),
            Storage::Reference(pointer) => {
                self.owned_references.insert(*pointer);
            }
            _ => unreachable!("an aggregate owner has storage"),
        }
        if self.holds_user_drop(ElementType::Struct(struct_id)) {
            let key = owner(storage).expect("an owner");
            let flag = self.place("$live", TypeName::Bool, true);
            self.drop_flags.insert(key, flag);
            self.set_live(key, true);
        }
    }

    pub(super) fn set_live(&mut self, key: Owner, live: bool) {
        if let Some(flag) = self.drop_flags.get(&key).copied() {
            let value = hir::Operand::Constant(BOOL, if live { -1 } else { 0 });
            self.emit("store", Vec::new(), vec![hir::Operand::Place(flag), value], None);
        }
    }

    /// The owner `view` is the whole of, if any.
    pub(super) fn whole_owner(view: &StructView) -> Option<Owner> {
        if view.offset != 0 || !view.indices.is_empty() {
            return None;
        }
        Some(match view.pointer {
            Some(pointer) => (true, pointer),
            None => (false, view.place),
        })
    }

    /// Drops what `view` owns, if its owner is still live.
    pub(super) fn drop_owner(&mut self, view: &StructView) {
        let flag = Self::whole_owner(view).and_then(|key| self.drop_flags.get(&key).copied());
        let Some(flag) = flag else {
            self.drop_view(view);
            return;
        };
        let live = self.value(TypeName::Bool);
        self.emit("load", vec![live], vec![hir::Operand::Place(flag)], None);
        let done = self.block();
        self.branch_unless("ne", hir::Operand::Value(live), hir::Operand::Constant(BOOL, 0), done);
        self.drop_view(view);
        self.terminate(jump(done));
        self.current = done;
    }
}
