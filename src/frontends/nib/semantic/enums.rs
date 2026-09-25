//! Enums: a payload-free one is a scalar tag; one with payloads is a tag
//! followed by the variants' fields, which overlap.

use super::*;
use crate::frontends::nib::syntax::Enum;
use crate::frontends::nib::syntax::StructField;

#[derive(Clone, Debug)]
pub(super) struct EnumLayout {
    pub(super) name: String,
    /// The tag's storage type: `u8` or `u16`.
    pub(super) tag: TypeName,
    /// The tag's width in a `bits` struct: declared as `uN`, else its storage's.
    pub(super) bits: u32,
    /// What a field or binding of this enum holds.
    pub(super) element: ElementType,
    pub(super) variants: Vec<VariantLayout>,
}

#[derive(Clone, Debug)]
pub(super) struct VariantLayout {
    pub(super) name: String,
    pub(super) tag: i64,
    pub(super) fields: Vec<(String, FieldLayout)>,
}

pub(super) const TAG: &str = "$tag";

impl EnumLayout {
    pub(super) fn variant(&self, name: &str, span: Span) -> Result<&VariantLayout, Diagnostic> {
        self.variants
            .iter()
            .find(|one| one.name == name)
            .ok_or_else(|| Diagnostic::new(span, format!("{} has no variant {name:?}", self.name)))
    }

    pub(super) fn scalar(&self) -> Option<TypeName> {
        match self.element {
            ElementType::Scalar(type_name) => Some(type_name),
            ElementType::Struct(_) => None,
        }
    }
}

/// The named types a declaration's fields refer to.
fn references<'a>(fields: impl Iterator<Item = &'a StructField>) -> Vec<&'a str> {
    fields
        .flat_map(|field| generics::named_in(&field.type_spec))
        .collect()
}

impl TypeRegistry {
    /// Registers structs and enums once everything each one names is known,
    /// so declaration order does not matter.
    pub(super) fn register_aggregates(
        &mut self,
        structs: &[Struct],
        enums: &[Enum],
    ) -> Result<(), Diagnostic> {
        let names: BTreeSet<&str> = structs
            .iter()
            .map(|one| one.name.as_str())
            .chain(enums.iter().map(|one| one.name.as_str()))
            .collect();
        for one in structs {
            let fixed = one.fields.iter().filter(|field| !field.mutable);
            self.fixed_fields.extend(fixed.map(|field| (one.name.clone(), field.name.clone())));
        }
        for one in structs.iter().filter(|one| !one.generics.is_empty()) {
            self.templates
                .insert(one.name.clone(), generics::Template::Struct(one.clone()));
        }
        for one in enums.iter().filter(|one| !one.generics.is_empty()) {
            self.templates
                .insert(one.name.clone(), generics::Template::Enum(one.clone()));
        }
        let mut pending_structs: Vec<&Struct> = structs
            .iter()
            .filter(|one| one.generics.is_empty())
            .collect();
        let mut pending_enums: Vec<&Enum> =
            enums.iter().filter(|one| one.generics.is_empty()).collect();
        while !pending_structs.is_empty() || !pending_enums.is_empty() {
            let ready = |references: Vec<&str>, types: &Self| {
                references
                    .into_iter()
                    .all(|name| !names.contains(name) || types.declared(name))
            };
            let before = pending_structs.len() + pending_enums.len();
            let mut index = 0;
            while index < pending_structs.len() {
                if ready(references(pending_structs[index].fields.iter()), self) {
                    self.register_struct(pending_structs.remove(index))?;
                } else {
                    index += 1;
                }
            }
            let mut index = 0;
            while index < pending_enums.len() {
                let fields = pending_enums[index]
                    .variants
                    .iter()
                    .flat_map(|one| &one.fields);
                if ready(references(fields), self) {
                    self.register_enum(pending_enums.remove(index))?;
                } else {
                    index += 1;
                }
            }
            if pending_structs.len() + pending_enums.len() == before {
                let span = pending_structs
                    .first()
                    .map(|one| one.span)
                    .or_else(|| pending_enums.first().map(|one| one.span))
                    .expect("something is pending");
                return Err(Diagnostic::new(span, "a type contains itself"));
            }
        }
        Ok(())
    }

    pub(super) fn register_enum(&mut self, declaration: &Enum) -> Result<(), Diagnostic> {
        if self.declared(&declaration.name) {
            return Err(Diagnostic::new(
                declaration.span,
                format!("type {:?} is declared more than once", declaration.name),
            ));
        }
        let mut tags = BTreeSet::new();
        let mut next = 0_i64;
        let mut variants = Vec::new();
        for variant in &declaration.variants {
            let tag = variant.tag.unwrap_or(next);
            if !tags.insert(tag)
                || variants
                    .iter()
                    .any(|one: &VariantLayout| one.name == variant.name)
            {
                return Err(Diagnostic::new(
                    variant.span,
                    format!("variant {:?} repeats a name or tag", variant.name),
                ));
            }
            next = tag + 1;
            variants.push(VariantLayout {
                name: variant.name.clone(),
                tag,
                fields: Vec::new(),
            });
        }
        let highest = *tags.last().expect("at least one variant");
        let (tag, bits) = match declaration.backing {
            Some(backing) => backing,
            None if highest <= 0xFF => (TypeName::U8, 8),
            None => (TypeName::U16, 16),
        };
        let limit = (1_i64 << bits) - 1;
        if *tags.first().expect("at least one variant") < 0 || highest > limit {
            return Err(Diagnostic::new(
                declaration.span,
                format!("a tag of {} does not fit its type", declaration.name),
            ));
        }
        let payload = declaration
            .variants
            .iter()
            .any(|one| !one.fields.is_empty());
        let element = if payload {
            let base = width(tag);
            let mut fields = BTreeMap::from([(
                TAG.to_owned(),
                FieldLayout {
                    type_: ElementType::Scalar(tag),
                    offset: 0,
                    shape: None,
                },
            )]);
            let mut size = base;
            // The bytes a payload's arrays take, as (start, end).
            let mut arrays = Vec::new();
            for (variant, layout) in declaration.variants.iter().zip(&mut variants) {
                let mut offset = align_up(base, 2);
                for field in &variant.fields {
                    let (field_layout, _, units) = self.place_field(field, offset, 2)?;
                    fields.insert(format!("${}.{}", variant.name, field.name), field_layout);
                    layout.fields.push((field.name.clone(), field_layout));
                    arrays.extend(units.into_iter().filter(|(_, _, count)| *count > 1).map(|(start, type_name, count)| (start, start + width(type_name) * count)));
                    offset = field_layout.offset + self.field_width(field_layout);
                }
                size = size.max(offset);
            }
            let size = align_up(size, 2);
            // Whole words, since the variants overlap; the words inside a
            // payload's array are one run, as the array's copy is.
            let mut copy: Vec<(u32, TypeName, u32)> = Vec::new();
            let mut previous = None;
            for word in (0..size).step_by(2) {
                let within = arrays.iter().copied().find(|(start, end)| *start <= word && word + 2 <= *end);
                match copy.last_mut() {
                    Some((_, _, count)) if within.is_some() && within == previous => *count += 1,
                    _ => copy.push((word, TypeName::U16, 1)),
                }
                previous = within;
            }
            ElementType::Struct(self.aggregate(
                &declaration.name,
                size,
                fields,
                vec![TAG.to_owned()],
                copy,
            ))
        } else {
            let type_id = self.types.len() as u32 + 1;
            self.types.push(plain_type(
                type_id,
                &declaration.name,
                "integer",
                width(tag),
                Some(false),
                "none",
            ));
            ElementType::Scalar(TypeName::Enum {
                type_id,
                width: width(tag) as u8,
            })
        };
        self.enums.insert(
            declaration.name.clone(),
            EnumLayout {
                name: declaration.name.clone(),
                tag,
                bits,
                element,
                variants,
            },
        );
        Ok(())
    }

    /// The enum a scalar or layout belongs to.
    pub(super) fn enum_of(&self, element: ElementType) -> Option<&EnumLayout> {
        self.enums.values().find(|one| one.element == element)
    }
}

impl FunctionCompiler<'_> {
    /// The enum a variant expression names, or the one `expected` is.
    fn variant_enum(
        &self,
        enum_name: Option<&str>,
        expected: Option<ElementType>,
        span: Span,
    ) -> Result<EnumLayout, Diagnostic> {
        let expected_layout = expected.and_then(|one| self.types.enum_of(one));
        match (enum_name, expected) {
            // A generic enum's variant builds whichever instance is expected.
            (Some(name), _) if expected_layout.is_some_and(|one| self.types.template_of(&one.name) == self.types.template_of(name)) => {
                Ok(expected_layout.expect("checked").clone())
            }
            (Some(name), _) => self
                .types
                .enums
                .get(name)
                .cloned()
                .ok_or_else(|| Diagnostic::new(span, format!("unknown enum {name:?}"))),
            (None, Some(element)) => self
                .types
                .enum_of(element)
                .cloned()
                .ok_or_else(|| Diagnostic::new(span, "a variant needs an expected enum type")),
            (None, None) => Err(Diagnostic::new(
                span,
                "a variant needs an expected enum type",
            )),
        }
    }

    /// A payload-free variant: its tag.
    pub(super) fn scalar_variant(
        &mut self,
        enum_name: Option<&str>,
        name: &str,
        arguments: &[Expr],
        expected: Option<TypeName>,
        span: Span,
    ) -> Result<TypedOperand, Diagnostic> {
        let layout = self.variant_enum(enum_name, expected.map(ElementType::Scalar), span)?;
        let Some(type_name) = layout.scalar() else {
            return Err(Diagnostic::new(
                span,
                format!("{} carries a payload and must be bound first", layout.name),
            ));
        };
        if expected.is_some_and(|one| one != type_name) {
            return Err(type_mismatch(span, expected.expect("checked"), type_name));
        }
        if !arguments.is_empty() {
            return Err(Diagnostic::new(
                span,
                format!("{}.{name} carries no payload", layout.name),
            ));
        }
        let tag = layout.variant(name, span)?.tag;
        Ok(TypedOperand {
            operand: Some(hir::Operand::Constant(type_id(type_name), tag)),
            type_name,
        })
    }

    /// Writes a variant's tag and payload into an enum's storage.
    pub(super) fn prepare_variant_stores(
        &mut self,
        destination: &StructView,
        enum_name: Option<&str>,
        name: &str,
        arguments: &[Expr],
        span: Span,
        stores: &mut Vec<Store>,
    ) -> Result<(), Diagnostic> {
        let layout = self.variant_enum(
            enum_name,
            Some(ElementType::Struct(destination.struct_id)),
            span,
        )?;
        if layout.element != ElementType::Struct(destination.struct_id) {
            let expected = &self
                .types
                .structure(destination.struct_id)
                .expect("resolved layout")
                .name;
            return Err(Diagnostic::new(
                span,
                format!("expected {expected}, found {}", layout.name),
            ));
        }
        let variant = layout.variant(name, span)?.clone();
        stores.push(Store::One(
            self.projected_place(destination, 0, layout.tag),
            hir::Operand::Constant(type_id(layout.tag), variant.tag),
        ));
        let formals: Vec<_> = variant
            .fields
            .iter()
            .map(|(field, _)| Formal {
                name: field,
                default: None,
            })
            .collect();
        let values = arguments::bind(
            &format!("{}.{name}", layout.name),
            &formals,
            arguments.to_vec(),
            span,
        )?;
        for ((_, field), value) in variant.fields.iter().zip(&values) {
            self.prepare_field_store(destination, *field, value, value.span(), stores)?;
        }
        Ok(())
    }
}
