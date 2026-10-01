//! `-g`: BASIC's side of the debug information -- its types and the names
//! BC's /Zi gives them. `llrm_hir::debug` builds the rest.

use llrm_hir::model::DebugScalar;

use super::{Compiler, DOUBLE, INTEGER, LONG, SIGNED_BYTE, SINGLE, STRING};
use crate::syntax::{Declaration, Span};

const SIGILS: [char; 6] = ['%', '&', '!', '#', '$', '@'];

/// BASIC's type suffix of a scalar type.
fn sigil(type_id: u32) -> Option<char> {
    match type_id {
        INTEGER => Some('%'),
        LONG => Some('&'),
        SINGLE => Some('!'),
        DOUBLE => Some('#'),
        STRING => Some('$'),
        _ => None,
    }
}

/// Where a variable is held.
pub(super) enum Held {
    Place(u32),
    /// `offset` bytes into a module data object.
    Data(u32, isize),
}

impl Compiler {
    /// `-g`.
    pub(super) fn debugging(&self) -> bool {
        self.source.as_ref().is_some_and(|source| source.debug)
    }

    /// Expanded line `line`'s in the main file, 0 for none.
    pub(super) fn main_line(&self, line: usize) -> usize {
        self.source.as_ref().and_then(|source| source.lines.get(line.wrapping_sub(1))).copied().unwrap_or(0)
    }

    /// `type_id`'s debug type, where it has one: a scalar, a TYPE, or a
    /// `STRING * n`.
    fn debug_type(&mut self, type_id: u32) -> Option<i64> {
        if let Some(&made) = self.debug_structures.get(&type_id) {
            return Some(made);
        }
        let scalar = match type_id {
            SIGNED_BYTE => DebugScalar::Int8,
            INTEGER => DebugScalar::Int16,
            LONG => DebugScalar::Int32,
            SINGLE => DebugScalar::Float32,
            DOUBLE => DebugScalar::Float64,
            STRING if self.runtime == "vbdos" => DebugScalar::FarString,
            STRING => DebugScalar::String,
            _ => return self.string_width(type_id).map(|width| self.debug.fixed_string(width as i64)),
        };
        Some(self.debug.scalar(scalar))
    }

    /// `type_id`'s, or an array of them.
    fn debug_type_of(&mut self, type_id: u32, array: bool) -> Option<i64> {
        let element = self.debug_type(type_id)?;
        Some(if array { self.debug.array(element) } else { element })
    }

    /// The TYPE `declared` at `span`, `type_id`, of `fields` at `offsets`,
    /// each of its type or `bytes` of it in place.
    pub(super) fn debug_structure(&mut self, type_id: u32, declared: &str, span: Span, fields: &[(&Declaration, u32, usize, usize)], bytes: usize) {
        if !self.debugging() {
            return;
        }
        let mut members = Vec::new();
        for &(field, field_type, offset, extent) in fields {
            let Some(mut r#type) = self.debug_type(field_type) else { return };
            if !field.bounds.is_empty() {
                r#type = self.debug.sized(r#type, extent as i64);
            }
            members.push((self.debug_name(&field.name, field.span, 0), r#type, offset as i64));
        }
        let name = self.debug_name(declared, span, 0);
        let fields: Vec<(&str, i64, i64)> = members.iter().map(|(name, r#type, offset)| (name.as_str(), *r#type, *offset)).collect();
        let made = self.debug.structure(&name, bytes as i64, &fields);
        self.debug_structures.insert(type_id, made);
    }

    /// `name`, declared at `span`, as BC's /Zi names it: VBDOS keeps the
    /// source's spelling and drops the suffix; QB 4.5 and PDS upper-case it
    /// and suffix a scalar.
    fn debug_name(&self, name: &str, span: Span, type_id: u32) -> String {
        let bare = name.trim_end_matches(SIGILS);
        if self.runtime == "vbdos" {
            return self.spelled(bare, span);
        }
        let mut name = bare.to_ascii_uppercase();
        name.extend(sigil(type_id));
        name
    }

    /// `name` as the source spells it at or after `span`, a whole word;
    /// the lexer upper-cases every identifier.
    fn spelled(&self, name: &str, span: Span) -> String {
        let line = self.source.as_ref().and_then(|source| source.text.get(span.line.wrapping_sub(1)));
        let found = line.and_then(|line| {
            let upper = line.to_ascii_uppercase();
            let bytes = upper.as_bytes();
            let word = |at: usize| at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_' || bytes[at] == b'.');
            let from = span.start.min(line.len());
            let at = upper
                .get(from..)?
                .match_indices(&name.to_ascii_uppercase())
                .map(|(at, _)| from + at)
                .find(|&at| (at == 0 || !word(at - 1)) && !word(at + name.len()))?;
            Some(line[at..at + name.len()].to_owned())
        });
        found.unwrap_or_else(|| name.to_owned())
    }

    /// A variable `source` declared at `span`, of `type_id`s or an array
    /// of them, `held` there; an array by its descriptor, as BC's.
    pub(super) fn debug_held(&mut self, held: Held, source: &str, span: Span, type_id: u32, array: bool) {
        if !self.debugging() || source.starts_with('$') {
            return;
        }
        let Some(r#type) = self.debug_type_of(type_id, array) else { return };
        let name = self.debug_name(source, span, type_id);
        match held {
            Held::Place(place) => self.debug.variable(place.into(), &name, r#type),
            Held::Data(object, offset) => self.debug.global(object.into(), offset as i64, &name, r#type),
        }
    }

    /// `place` holds no source variable: a copy of a parameter, or a result.
    pub(super) fn debug_forget(&mut self, place: u32) {
        self.debug.forget(place.into());
    }

    /// The function's `argument`th parameter, `source` declared at `span`,
    /// of `type_id`s or an array of them, by reference unless `by_value`.
    pub(super) fn debug_parameter(&mut self, argument: usize, source: &str, span: Span, type_id: u32, array: bool, by_value: bool) {
        if !self.debugging() {
            return;
        }
        let Some(target) = self.debug_type_of(type_id, array) else { return };
        let r#type = if by_value { target } else { self.debug.reference(target) };
        let name = self.debug_name(source, span, type_id);
        self.debug.parameter(argument as i64, &name, r#type);
    }

    /// The function `id` just compiled, `source` declared at `span`
    /// returning `result`, None for a SUB.
    pub(super) fn debug_function(&mut self, id: u32, source: &str, span: Span, result: Option<u32>) {
        if !self.debugging() {
            return;
        }
        // BC gives a SUB an INTEGER result.
        let result_type = self.debug_type(result.unwrap_or(INTEGER));
        let name = self.debug_name(source, span, result.unwrap_or(0));
        self.debug.function(id.into(), &name, result_type);
    }
}
