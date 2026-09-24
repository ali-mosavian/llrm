//! Sequence patterns (draft section 6): `[a, *rest, z]` against a borrowed
//! view of a vec, array or view. Element bindings read their elements; the
//! starred one is a view of what lies between. Nothing is removed.

use super::matching::Subject;
use super::vectors::GENERATED;
use super::*;
use crate::frontends::modern::syntax::Pattern;

impl FunctionCompiler<'_> {
    /// `expression`, borrowed as a one-dimensional view to match against.
    pub(super) fn sequence_subject(&mut self, expression: &Expr, span: Span) -> Result<Subject, Diagnostic> {
        let element = self
            .iterated_item(expression)
            .ok_or_else(|| Diagnostic::new(span, "a sequence pattern needs a vec, array or view"))?;
        let target = BindingType::Slice { element, rank: 1 };
        let pointer_type = self.types.slice_pointer(element, 1);
        let borrow = Expr::Borrow { mutable: false, operand: Box::new(expression.clone()), span };
        let (descriptor, _) = self.borrow_argument(&borrow, false, target, pointer_type)?;
        let hir::Operand::Value(descriptor) = descriptor else {
            unreachable!("a borrowed view is a descriptor pointer")
        };
        let (data, length) = self.view_parts_of(descriptor, element);
        Ok(Subject::Sequence { descriptor, data, length, element })
    }

    /// Branches to `fail` unless the sequence has as many elements as the
    /// pattern names, and each named one matches.
    pub(super) fn test_sequence(
        &mut self,
        (before, rest, after): (&[Pattern], bool, &[Pattern]),
        subject: &Subject,
        fail: u32,
    ) -> Result<(), Diagnostic> {
        let Subject::Sequence { length, .. } = subject else {
            unreachable!("a sequence subject")
        };
        let named = hir::Operand::Constant(U16, (before.len() + after.len()) as i64);
        self.branch_unless(if rest { "ge" } else { "eq" }, length.clone(), named, fail);
        for (pattern, inner) in self.sequence_parts(before, after, subject)? {
            self.test(pattern, &inner, fail)?;
        }
        Ok(())
    }

    /// Binds a sequence pattern already known to match.
    pub(super) fn bind_sequence(
        &mut self,
        (before, rest, after): (&[Pattern], Option<&Pattern>, &[Pattern]),
        subject: &Subject,
    ) -> Result<(), Diagnostic> {
        for (pattern, inner) in self.sequence_parts(before, after, subject)? {
            self.bind(pattern, &inner)?;
        }
        let Some(Pattern::Binding(name, span)) = rest else {
            return Ok(());
        };
        let Subject::Sequence { data, length, element, .. } = subject else {
            unreachable!("a sequence subject")
        };
        let first = self.nth_element(*data, *element, hir::Operand::Constant(U16, before.len() as i64))?;
        let count = self.value(TypeName::U16);
        let named = hir::Operand::Constant(U16, (before.len() + after.len()) as i64);
        self.emit("sub", vec![count], vec![length.clone(), named], None);
        let pointer_type = self.types.slice_pointer(*element, 1);
        let count = hir::Operand::Value(count);
        let hir::Operand::Value(descriptor) = self.view_descriptor(name, pointer_type, vec![count.clone(), count], first)
        else {
            unreachable!("a descriptor pointer")
        };
        let binding = Binding {
            type_: BindingType::Slice { element: *element, rank: 1 },
            mutable: false,
            storage: Storage::Slice(descriptor),
        };
        if self.scopes.last_mut().expect("scope").insert(name.clone(), binding).is_some() {
            return Err(Diagnostic::new(*span, format!("{name:?} is bound twice in one pattern")));
        }
        Ok(())
    }

    /// Each element pattern with the element it names: `before` from the
    /// front, `after` from the back.
    fn sequence_parts<'p>(
        &mut self,
        before: &'p [Pattern],
        after: &'p [Pattern],
        subject: &Subject,
    ) -> Result<Vec<(&'p Pattern, Subject)>, Diagnostic> {
        let Subject::Sequence { data, length, element, .. } = subject else {
            unreachable!("a sequence subject")
        };
        let mut parts = Vec::new();
        for (index, pattern) in before.iter().enumerate() {
            let pointer = self.nth_element(*data, *element, hir::Operand::Constant(U16, index as i64))?;
            parts.push((pattern, self.element_subject(pointer, *element)));
        }
        for (back, pattern) in after.iter().enumerate() {
            let index = self.value(TypeName::U16);
            let from_end = hir::Operand::Constant(U16, (after.len() - back) as i64);
            self.emit("sub", vec![index], vec![length.clone(), from_end], None);
            let pointer = self.nth_element(*data, *element, hir::Operand::Value(index))?;
            parts.push((pattern, self.element_subject(pointer, *element)));
        }
        Ok(parts)
    }

    fn nth_element(&mut self, data: u32, element: ElementType, index: hir::Operand) -> Result<u32, Diagnostic> {
        let width = self.types.width(element.id());
        self.indexed_pointer(data, index, width, GENERATED)
    }

    /// The element `pointer` addresses: a scalar is read, a struct viewed.
    fn element_subject(&mut self, pointer: u32, element: ElementType) -> Subject {
        match element {
            ElementType::Scalar(type_name) => {
                let value = self.value(type_name);
                let place = hir::Operand::IndirectPlace { base: pointer, offset: 0, type_id: type_id(type_name), inbounds: false };
                self.emit("load", vec![value], vec![place.clone()], None);
                Subject::Scalar(hir::Operand::Value(value), type_name, Some(place))
            }
            ElementType::Struct(struct_id) => Subject::Aggregate(StructView {
                struct_id,
                place: 0,
                pointer: Some(pointer),
                indices: Vec::new(),
                offset: 0,
                mutable: false,
                owner: format!("$element{pointer}"),
            }),
        }
    }
}
