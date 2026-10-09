//! User `drop` methods (section 8): `fn T.drop(self: &mut T)` runs once
//! when an owner of a T ends, before what T's fields own is dropped. A
//! moved-from T has no null to mark it, so each owning local of a type that
//! holds one carries a flag, cleared when its value moves.

use super::moves::{Owner, owner};
use super::*;

impl TypeRegistry {
    /// Records each struct's `drop` method, checking its form.
    pub(super) fn register_drops(
        &mut self,
        functions: &[&Function],
    ) -> Result<(), Diagnostic> {
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
                return Err(Diagnostic::new(
                    function.span,
                    format!("a drop method is 'fn {type_name}.drop(self: &mut {type_name}) -> void'"),
                ));
            }
            self.dropped.insert(layout.id, function.name.clone());
        }
        Ok(())
    }
}

impl FunctionCompiler<'_> {
    /// Whether a value of this type holds something with a `drop` method.
    pub(super) fn holds_user_drop(
        &self,
        element: ElementType,
    ) -> bool {
        match element {
            ElementType::Scalar(type_name) => {
                self.types.sequence_element(type_name).is_some_and(|inner| self.holds_user_drop(inner))
            }
            ElementType::Struct(id) if self.types.array_of(id).is_some() => {
                self.holds_user_drop(self.types.array_of(id).expect("an array").0)
            }
            ElementType::Struct(id) => {
                self.types.dropped.contains_key(&id)
                    || self
                        .types
                        .structure(id)
                        .expect("registered layout")
                        .fields
                        .values()
                        .any(|field| self.holds_user_drop(field.type_))
            }
        }
    }

    /// Refuses to duplicate what holds a resource.
    pub(super) fn check_copyable(
        &self,
        element: ElementType,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if self.holds_user_drop(element) {
            return Err(Diagnostic::new(span, "a value holding a type with a drop method cannot be copied"));
        }
        Ok(())
    }

    /// Calls the `drop` method of `view`'s type, if it has one.
    pub(super) fn call_drop(
        &mut self,
        view: &StructView,
    ) {
        let Some(name) = self.types.dropped.get(&view.struct_id).cloned() else {
            return;
        };
        let callee = self.signatures[&name].id;
        let pointer = self.address_of(view);
        let instruction = self.emit("call", Vec::new(), vec![pointer], Some(name));
        self.calls.push(hir::CallSite::new(instruction, callee, 1, self.types.native));
    }

    pub(super) fn is_drop_method(
        &self,
        name: &str,
    ) -> bool {
        self.types.dropped.values().any(|one| one == name)
    }

    /// Makes `storage` an owner of a `struct_id`, flagged live when it holds a `drop`.
    pub(super) fn own_aggregate(
        &mut self,
        storage: &Storage,
        struct_id: u32,
    ) {
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

    pub(super) fn set_live(
        &mut self,
        key: Owner,
        live: bool,
    ) {
        if let Some(flag) = self.drop_flags.get(&key).copied() {
            let value = hir::Operand::Constant(BOOL, i64::from(live));
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

    /// Drops what `view` owns, if its owner is still live, but for the
    /// fields with a `drop` that moved out of it.
    pub(super) fn drop_owner(
        &mut self,
        view: &StructView,
    ) {
        let key = Self::whole_owner(view);
        let flag = key.and_then(|key| self.drop_flags.get(&key).copied());
        let fates = match key {
            Some(key) => self.fates(key),
            None => Vec::new(),
        };
        self.when_live(flag.map(hir::Operand::Place), |this| this.drop_except(view, &fates));
    }

    /// What became of each field of `owner` with a `drop` that moved: gone
    /// on every path here, or on some, as its flag says.
    fn fates(
        &mut self,
        owner: Owner,
    ) -> Vec<(Vec<String>, Option<u32>)> {
        let dropped: BTreeSet<Vec<String>> = self
            .field_moves
            .iter()
            .filter(|((one, _), ..)| *one == owner)
            .map(|((_, path), ..)| path.clone())
            .collect();
        let moved: Vec<Vec<String>> =
            self.moved_paths(owner).into_iter().filter(|path| dropped.contains(path)).collect();
        moved
            .into_iter()
            .map(|path| match self.surely_moved(owner, &path) {
                true => (path, None),
                false => {
                    let flag = self.field_flag(owner, &path);
                    (path, Some(flag))
                }
            })
            .collect()
    }

    /// `drop_view` of `view`, leaving out each field `fates` says moved, or
    /// asking its flag, which it sets again for the next owner there.
    fn drop_except(
        &mut self,
        view: &StructView,
        fates: &[(Vec<String>, Option<u32>)],
    ) {
        if fates.is_empty() {
            return self.drop_view(view);
        }
        // Nothing moves out of a struct with a `drop`: it has none to call.
        let layout = self.types.structure(view.struct_id).expect("registered layout").clone();
        for (name, field) in &layout.fields {
            let below: Vec<(Vec<String>, Option<u32>)> = fates
                .iter()
                .filter(|(path, _)| path[0] == *name)
                .map(|(path, flag)| (path[1..].to_vec(), *flag))
                .collect();
            match below.iter().find(|(path, _)| path.is_empty()) {
                Some((_, None)) => {}
                Some((_, Some(flag))) => {
                    let flag = hir::Operand::Place(*flag);
                    self.when_live(Some(flag.clone()), |this| this.owned_field(view, *field, Owned::Drop));
                    self.emit("store", Vec::new(), vec![flag, hir::Operand::Constant(BOOL, 1)], None);
                }
                None if below.is_empty() => self.owned_field(view, *field, Owned::Drop),
                None => {
                    let ElementType::Struct(struct_id) = field.type_ else {
                        unreachable!("a path goes through structs")
                    };
                    let inner = StructView { struct_id, offset: view.offset + field.offset, ..view.clone() };
                    self.drop_except(&inner, &below);
                }
            }
        }
    }

    /// The flag of the field `path` of `owner`, made where a drop asks it:
    /// set at the function's start, cleared by each move (`clear_moved`),
    /// and set again after each drop that asks it.
    fn field_flag(
        &mut self,
        owner: Owner,
        path: &[String],
    ) -> u32 {
        if let Some(flag) = self.field_flags.get(&(owner, path.to_vec())) {
            return *flag;
        }
        let flag = self.place("$moved", TypeName::Bool, true);
        self.field_flags.insert((owner, path.to_vec()), flag);
        flag
    }

    /// Clears each flag a drop asks where its field moves, and sets it at
    /// the start; a move whose drops all knew it moved needs none.
    pub(super) fn clear_moved(&mut self) {
        let mut stores: Vec<(u32, usize, u32, i64)> = std::mem::take(&mut self.field_moves)
            .into_iter()
            .filter_map(|(key, block, at)| Some((block, at, *self.field_flags.get(&key)?, 0)))
            .collect();
        for ((owner, filled), block, at) in std::mem::take(&mut self.field_refills) {
            let flags = self.field_flags.iter().filter(|((one, path), _)| *one == owner && path.starts_with(&filled));
            stores.extend(flags.map(|(_, flag)| (block, at, *flag, 1)));
        }
        stores.extend(self.field_flags.values().map(|flag| (1, 0, *flag, 1)));
        // From the end of each block back, so that each index still holds.
        stores.sort_by(|one, other| (other.0, other.1).cmp(&(one.0, one.1)).then(one.3.cmp(&other.3)));
        for (block, at, flag, value) in stores {
            let id = self.next_instruction;
            self.next_instruction += 1;
            let store = hir::Instruction {
                id,
                op: "store",
                results: Vec::new(),
                operands: vec![hir::Operand::Place(flag), hir::Operand::Constant(BOOL, value)],
                callee: None,
                asm: None,
                inbounds: false,
                line: 0,
            };
            self.blocks[(block - 1) as usize].instructions.insert(at, store);
        }
    }

    /// Emits `then` to run only while `flag`, when there is one, is set.
    pub(super) fn when_live(
        &mut self,
        flag: Option<hir::Operand>,
        then: impl FnOnce(&mut Self),
    ) {
        let Some(flag) = flag else {
            return then(self);
        };
        let live = self.value(TypeName::Bool);
        self.emit("load", vec![live], vec![flag], None);
        let done = self.block();
        self.branch_unless("ne", hir::Operand::Value(live), hir::Operand::Constant(BOOL, 0), done);
        then(self);
        self.terminate(jump(done));
        self.current = done;
    }
}
