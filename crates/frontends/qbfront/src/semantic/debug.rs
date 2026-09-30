//! `-g`: BASIC's side of the debug information -- its types and the names
//! BC's /Zi gives them. `llrm_hir::debug` builds the rest.

use llrm_hir::model::DebugScalar;

use super::{Compiler, DOUBLE, INTEGER, LONG, SINGLE, STRING};
use crate::syntax::Span;

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

impl Compiler {
    /// `type_id`'s debug type, where it has one yet.
    fn debug_type(&mut self, type_id: u32) -> Option<i64> {
        let scalar = match type_id {
            INTEGER => DebugScalar::Int16,
            LONG => DebugScalar::Int32,
            SINGLE => DebugScalar::Float32,
            DOUBLE => DebugScalar::Float64,
            STRING if self.runtime == "vbdos" => DebugScalar::FarString,
            STRING => DebugScalar::String,
            _ => return None,
        };
        Some(self.debug.scalar(scalar))
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

    /// A variable `source` declared at `span`, held in `place`.
    pub(super) fn debug_variable(&mut self, place: u32, source: &str, span: Span, type_id: u32) {
        if self.source.is_none() || source.starts_with('$') {
            return;
        }
        if let Some(r#type) = self.debug_type(type_id) {
            let name = self.debug_name(source, span, type_id);
            self.debug.variable(place.into(), &name, r#type);
        }
    }

    /// A module variable `source` declared at `span`, `offset` bytes into
    /// data object `object`.
    pub(super) fn debug_global(&mut self, object: u32, offset: isize, source: &str, span: Span, type_id: u32) {
        if self.source.is_none() || source.starts_with('$') {
            return;
        }
        if let Some(r#type) = self.debug_type(type_id) {
            let name = self.debug_name(source, span, type_id);
            self.debug.global(object.into(), offset as i64, &name, r#type);
        }
    }

    /// `place` holds no source variable: a copy of a parameter, or a result.
    pub(super) fn debug_forget(&mut self, place: u32) {
        self.debug.forget(place.into());
    }

    /// The function's `argument`th parameter, `source` declared at `span`,
    /// by reference unless `by_value`.
    pub(super) fn debug_parameter(&mut self, argument: usize, source: &str, span: Span, type_id: u32, by_value: bool) {
        if self.source.is_none() {
            return;
        }
        let Some(target) = self.debug_type(type_id) else { return };
        let r#type = if by_value { target } else { self.debug.reference(target) };
        let name = self.debug_name(source, span, type_id);
        self.debug.parameter(argument as i64, &name, r#type);
    }

    /// The function `id` just compiled, `source` declared at `span`
    /// returning `result`, None for a SUB.
    pub(super) fn debug_function(&mut self, id: u32, source: &str, span: Span, result: Option<u32>) {
        if self.source.is_none() {
            return;
        }
        // BC gives a SUB an INTEGER result.
        let result_type = self.debug_type(result.unwrap_or(INTEGER));
        let name = self.debug_name(source, span, result.unwrap_or(0));
        self.debug.function(id.into(), &name, result_type);
    }
}
