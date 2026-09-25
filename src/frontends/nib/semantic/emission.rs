//! Bindings, places, and the HIR a function is built from.

use super::*;

impl<'a> FunctionCompiler<'a> {
    /// A name no source can write and no other hidden name has: `$stem` and a number.
    pub(super) fn hidden(&mut self, stem: &str) -> String {
        self.next_hidden += 1;
        format!("${stem}{}", self.next_hidden)
    }

    /// Calls a runtime routine; its result, when it has one.
    pub(super) fn emit_builtin(
        &mut self,
        name: &'static str,
        operands: Vec<hir::Operand>,
    ) -> Option<hir::Operand> {
        let count = operands.len();
        let (callee, result) = *self
            .builtin_ids
            .get(name)
            .expect("registered runtime routine");
        // The runtime serves every heap sequence as a string.
        let operands = operands
            .into_iter()
            .map(|operand| match operand {
                hir::Operand::Value(value)
                    if self.types.vectors.contains_key(&self.type_of(value)) =>
                {
                    let text = self.value(TypeName::String);
                    self.emit("copy", vec![text], vec![operand], None);
                    hir::Operand::Value(text)
                }
                other => other,
            })
            .collect::<Vec<_>>();
        let results: Vec<u32> = (result != TypeName::Void)
            .then(|| self.value(result))
            .into_iter()
            .collect();
        let instruction = self.emit("call", results.clone(), operands, Some(name.into()));
        self.calls.push(hir::CallSite::new(instruction, callee, count as u32, Abi::Cdecl16));
        results.first().map(|one| hir::Operand::Value(*one))
    }

    pub(super) fn binding(&self, name: &str, span: Span) -> Result<&Binding, Diagnostic> {
        let binding = self
            .visible(name)
            .ok_or_else(|| Diagnostic::new(span, format!("unknown name {name:?}")))?;
        self.learn(span, name, || Known::Local(self.spelled(binding.type_)));
        self.check_unmoved(name, binding, span)?;
        Ok(binding)
    }

    /// The innermost binding of `name` outside the hidden scopes.
    pub(super) fn visible(&self, name: &str) -> Option<&Binding> {
        self.scopes
            .iter()
            .enumerate()
            .rev()
            .filter(|(index, _)| !self.hidden.iter().any(|range| range.contains(index)))
            .find_map(|(_, scope)| scope.get(name))
    }

    pub(super) fn type_of(&self, value: u32) -> u32 {
        self.values
            .iter()
            .find(|one| one.id == value)
            .expect("a declared value")
            .type_id
    }

    /// `operand` as a value: a constant is copied into one.
    pub(super) fn materialized(&mut self, operand: hir::Operand, type_id: u32) -> u32 {
        match operand {
            hir::Operand::Value(value) => value,
            other => {
                let value = self.value_type(type_id);
                self.emit("copy", vec![value], vec![other], None);
                value
            }
        }
    }

    pub(super) fn value(&mut self, type_name: TypeName) -> u32 {
        self.value_type(type_id(type_name))
    }

    pub(super) fn value_type(&mut self, type_id: u32) -> u32 {
        let id = self.next_value;
        self.next_value += 1;
        self.values.push(hir::Value { id, type_id });
        id
    }

    pub(super) fn place(&mut self, name: &str, type_name: TypeName, mutable: bool) -> u32 {
        self.local_place(name, type_id(type_name), width(type_name), mutable)
    }

    pub(super) fn local_place(&mut self, name: &str, type_id: u32, extent: u32, mutable: bool) -> u32 {
        let id = self.next_place;
        self.next_place += 1;
        self.next_frame_offset -= extent as i32;
        self.places.push(hir::Place {
            id,
            name: name.into(),
            type_id,
            mutable,
            offset: self.next_frame_offset,
            extent,
            storage: "local",
            symbol: 0,
            volatile: false,
        });
        id
    }

    pub(super) fn array_place(
        &mut self,
        name: &str,
        type_id: u32,
        element: ElementType,
        shape: Shape,
        mutable: bool,
    ) -> u32 {
        let extent = self.types.width(element.id()) * shape.len();
        self.next_frame_offset -= (extent + descriptor::size(shape.rank)) as i32;
        let descriptor_offset = self.next_frame_offset;
        self.array_place_at(descriptor_offset, name, type_id, element, shape, mutable)
    }

    /// An array whose descriptor starts at `descriptor_offset`, its data right after.
    pub(super) fn array_place_at(
        &mut self,
        descriptor_offset: i32,
        name: &str,
        type_id: u32,
        element: ElementType,
        shape: Shape,
        mutable: bool,
    ) -> u32 {
        let extent = self.types.width(element.id()) * shape.len();
        let size = descriptor::size(shape.rank);
        let descriptor = shape.descriptor();
        for (word, (label, value)) in descriptor.into_iter().enumerate() {
            let place = self.next_place;
            self.next_place += 1;
            self.places.push(hir::Place {
                id: place,
                name: format!("${name}.{label}"),
                type_id: U16,
                mutable: false,
                offset: descriptor_offset + 2 * word as i32,
                extent: 2,
                storage: "local",
                symbol: 0,
                volatile: true,
            });
            self.emit(
                "store",
                Vec::new(),
                vec![
                    hir::Operand::Place(place),
                    hir::Operand::Constant(U16, i64::from(value)),
                ],
                None,
            );
        }
        let id = self.next_place;
        self.next_place += 1;
        self.places.push(hir::Place {
            id,
            name: name.into(),
            type_id,
            mutable,
            offset: descriptor_offset + size as i32,
            extent,
            storage: "local",
            symbol: 0,
            volatile: false,
        });
        id
    }

    pub(super) fn static_place(&mut self, symbol: u32, type_name: TypeName) -> u32 {
        let id = self.next_place;
        self.next_place += 1;
        self.places.push(hir::Place {
            id,
            name: format!("$literal{symbol}"),
            type_id: type_id(type_name),
            mutable: false,
            offset: 0,
            extent: width(type_name),
            storage: "module",
            symbol,
            volatile: false,
        });
        id
    }

    pub(super) fn static_string_place(&mut self, symbol: u32, extent: u32) -> u32 {
        let id = self.next_place;
        self.next_place += 1;
        self.places.push(hir::Place {
            id,
            name: format!("$string{symbol}"),
            type_id: CHAR,
            mutable: false,
            // The exported string address is the byte payload. Its flags, pad,
            // length, and capacity occupy the six bytes immediately before it.
            offset: 6,
            extent,
            storage: "module",
            symbol,
            volatile: false,
        });
        id
    }

    pub(super) fn emit(
        &mut self,
        op: &'static str,
        results: Vec<u32>,
        operands: Vec<hir::Operand>,
        callee: Option<String>,
    ) -> u32 {
        let id = self.next_instruction;
        self.next_instruction += 1;
        self.moved();
        self.current_block_mut()
            .instructions
            .push(hir::Instruction {
                id,
                op,
                results,
                operands,
                callee,
                asm: None,
            });
        id
    }

    pub(super) fn block(&mut self) -> u32 {
        let id = self.blocks.len() as u32 + 1;
        self.blocks.push(BlockBuilder {
            id,
            instructions: Vec::new(),
            terminator: None,
        });
        id
    }

    pub(super) fn terminate(&mut self, terminator: hir::Terminator) {
        self.flow_moves(&terminator.targets);
        let block = self.current_block_mut();
        assert!(
            block.terminator.is_none(),
            "semantic block terminated twice"
        );
        block.terminator = Some(terminator);
    }

    /// Whether any block jumps to `block`.
    pub(super) fn reached(&self, block: u32) -> bool {
        self.blocks.iter().any(|one| one.terminator.as_ref().is_some_and(|end| end.targets.contains(&block)))
    }

    pub(super) fn open(&self) -> bool {
        self.blocks[(self.current - 1) as usize]
            .terminator
            .is_none()
    }

    pub(super) fn current_block_mut(&mut self) -> &mut BlockBuilder {
        &mut self.blocks[(self.current - 1) as usize]
    }
}
