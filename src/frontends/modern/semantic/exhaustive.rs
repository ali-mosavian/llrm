//! Whether a match's patterns cover every value: the usefulness check of a
//! pattern matrix, one column per value still to be matched.

use super::*;
use crate::frontends::modern::syntax::Pattern;

/// A way to build a value of a type with finitely many shapes.
struct Constructor {
    /// How a pattern names it, as a witness prints it.
    name: String,
    fields: Vec<ElementType>,
}

impl FunctionCompiler<'_> {
    /// A value no row matches, spelled as a pattern, or `None` when the rows cover all.
    pub(super) fn uncovered(
        &self,
        rows: &[Vec<&Pattern>],
        columns: &[ElementType],
    ) -> Option<Vec<String>> {
        let Some((&column, rest)) = columns.split_first() else {
            return rows.is_empty().then(Vec::new);
        };
        let Some(constructors) = self.constructors(column) else {
            // Infinitely many values: only rows that match anything help.
            let default: Vec<Vec<&Pattern>> = rows
                .iter()
                .filter(|row| matches!(row[0], Pattern::Wildcard(_) | Pattern::Binding(..)))
                .map(|row| row[1..].to_vec())
                .collect();
            return self
                .uncovered(&default, rest)
                .map(|tail| [vec!["_".to_owned()], tail].concat());
        };
        for constructor in constructors {
            let arity = constructor.fields.len();
            let specialized: Vec<Vec<&Pattern>> = rows
                .iter()
                .filter_map(|row| {
                    let fields: Vec<&Pattern> = match row[0] {
                        Pattern::Wildcard(_) | Pattern::Binding(..) => vec![&WILDCARD; arity],
                        // A variant written without fields matches any payload.
                        Pattern::Variant { name, fields, .. }
                            if format!(".{name}") == constructor.name =>
                        {
                            if fields.is_empty() {
                                vec![&WILDCARD; arity]
                            } else {
                                fields.iter().collect()
                            }
                        }
                        Pattern::Literal(Expr::Boolean(value, _))
                            if value.to_string() == constructor.name =>
                        {
                            Vec::new()
                        }
                        Pattern::Struct { fields, .. } | Pattern::Tuple(fields, _) => {
                            fields.iter().collect()
                        }
                        _ => return None,
                    };
                    Some([fields, row[1..].to_vec()].concat())
                })
                .collect();
            let columns = [constructor.fields.clone(), rest.to_vec()].concat();
            if let Some(witness) = self.uncovered(&specialized, &columns) {
                let (inner, tail) = witness.split_at(arity);
                let head = if arity == 0 {
                    constructor.name
                } else {
                    format!("{}({})", constructor.name, inner.join(", "))
                };
                return Some([vec![head], tail.to_vec()].concat());
            }
        }
        None
    }

    /// The shapes of a type with finitely many, or `None`.
    fn constructors(&self, element: ElementType) -> Option<Vec<Constructor>> {
        if let Some(layout) = self.types.enum_of(element) {
            return Some(
                layout
                    .variants
                    .iter()
                    .map(|variant| Constructor {
                        name: format!(".{}", variant.name),
                        fields: variant.fields.iter().map(|(_, one)| one.type_).collect(),
                    })
                    .collect(),
            );
        }
        match element {
            ElementType::Scalar(TypeName::Bool) => Some(
                ["true", "false"]
                    .into_iter()
                    .map(|name| Constructor {
                        name: name.into(),
                        fields: Vec::new(),
                    })
                    .collect(),
            ),
            ElementType::Struct(id) => {
                let layout = self.types.structure(id)?;
                Some(vec![Constructor {
                    name: layout.name.clone(),
                    fields: layout
                        .order
                        .iter()
                        .map(|field| layout.fields[field].type_)
                        .collect(),
                }])
            }
            ElementType::Scalar(_) => None,
        }
    }
}

static WILDCARD: Pattern = Pattern::Wildcard(Span::new(0, 0, 0));
