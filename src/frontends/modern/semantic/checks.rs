//! Runtime checks that invoke the panic handler: an index at or past its
//! dimension (section 13), a shift count at or past its operand's width,
//! and a float outside the integer type it converts to (section 3). A constant index into a known dimension is checked
//! here instead, and `unsafe` code, which vouches for its indices, is not
//! checked; a check a loop's range already proves is the optimizer's to
//! fold.

use super::*;

impl FunctionCompiler<'_> {
    /// Checks each of `indices` against the view `descriptor`'s dimensions.
    pub(super) fn check_view_bounds(&mut self, descriptor: u32, indices: &[hir::Operand], span: Span) -> Result<(), Diagnostic> {
        if self.unsafe_depth > 0 {
            return Ok(());
        }
        for (axis, index) in indices.iter().enumerate() {
            let dim = self.value(TypeName::U16);
            let place = hir::Operand::IndirectPlace { base: descriptor, offset: descriptor::dim(axis as u8), type_id: U16, inbounds: false };
            self.emit("load", vec![dim], vec![place], None);
            self.check_bounds(index, hir::Operand::Value(dim), span)?;
        }
        Ok(())
    }

    /// Panics unless `index` is below `dim`, compared unsigned so a negative
    /// index fails too.
    pub(super) fn check_bounds(&mut self, index: &hir::Operand, dim: hir::Operand, span: Span) -> Result<(), Diagnostic> {
        self.check_below(index, dim, "_rt_panic_bounds", span)
    }

    /// Panics through `panic` unless `value` is below `limit`, compared
    /// unsigned at the wider of their widths.
    pub(super) fn check_below(&mut self, value: &hir::Operand, limit: hir::Operand, panic: &'static str, span: Span) -> Result<(), Diagnostic> {
        if let (hir::Operand::Constant(_, at), hir::Operand::Constant(_, length)) = (value, &limit) {
            if *at < 0 || at >= length {
                return Err(Diagnostic::new(span, format!("{at} is outside 0..{length}")));
            }
            return Ok(());
        }
        if self.unsafe_depth > 0 {
            return Ok(());
        }
        let wide = [value, &limit].iter().any(|one| matches!(one, hir::Operand::Value(id) if self.types.width(self.type_of(*id)) == 4));
        let unsigned = if wide { TypeName::U32 } else { TypeName::U16 };
        let [value, limit] = [value.clone(), limit].map(|one| self.unsigned(one, unsigned));
        let below = self.value(TypeName::Bool);
        self.emit("below", vec![below], vec![value, limit], None);
        self.panic_unless(hir::Operand::Value(below), panic);
        Ok(())
    }

    /// Panics unless the float `value` truncates into the integer `target`:
    /// strictly between its minimum less one and its maximum plus one.
    pub(super) fn check_truncation(&mut self, value: &hir::Operand, source: TypeName, target: TypeName, span: Span) -> Result<(), Diagnostic> {
        let bits = 8 * width(target);
        let signed = matches!(target, TypeName::I8 | TypeName::I16 | TypeName::I32);
        let (minimum, maximum) = if signed { (-(1i64 << (bits - 1)), (1i64 << (bits - 1)) - 1) } else { (0, (1i64 << bits) - 1) };
        // An f32 holds no value strictly between -2^31 - 1 and -2^31.
        let (lower, strict) = if source == TypeName::F32 && bits == 32 && signed { (minimum, false) } else { (minimum - 1, true) };
        let lower = self.float(&lower.to_string(), Some(source), span)?;
        let upper = self.float(&(maximum + 1).to_string(), Some(source), span)?;
        let [above, below, inside] = [(); 3].map(|_| self.value(TypeName::Bool));
        self.emit(if strict { "gt" } else { "ge" }, vec![above], vec![value.clone(), required(lower, span)?], None);
        self.emit("lt", vec![below], vec![value.clone(), required(upper, span)?], None);
        self.emit("and", vec![inside], vec![hir::Operand::Value(above), hir::Operand::Value(below)], None);
        self.panic_unless(hir::Operand::Value(inside), "_rt_panic_convert");
        Ok(())
    }

    /// Continues where `condition` holds; elsewhere calls `panic`, which never returns.
    pub(super) fn panic_unless(&mut self, condition: hir::Operand, panic: &'static str) {
        let inside = self.block();
        let outside = self.block();
        self.terminate(hir::Terminator { kind: "branch", operands: vec![condition], targets: vec![inside, outside] });
        self.current = outside;
        self.emit_builtin(panic, Vec::new());
        self.terminate(hir::Terminator { kind: "unreachable", operands: Vec::new(), targets: Vec::new() });
        self.current = inside;
    }

    /// `operand` as the unsigned `type_name`.
    fn unsigned(&mut self, operand: hir::Operand, type_name: TypeName) -> hir::Operand {
        match operand {
            hir::Operand::Constant(_, at) => hir::Operand::Constant(type_id(type_name), at),
            hir::Operand::Value(id) if self.type_of(id) == type_id(type_name) => operand,
            other => {
                let converted = self.value(type_name);
                self.emit("convert", vec![converted], vec![other], None);
                hir::Operand::Value(converted)
            }
        }
    }
}
