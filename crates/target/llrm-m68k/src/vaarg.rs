//! `va_arg` written out as the loads, the pointer step and the store the
//! pin's legalizer makes of it.
//!
//! # How it works
//! Clang writes a variadic read of this ABI as `%v = va_arg ptr %list, T`,
//! which MIR's reader does not take. [`without_va_arg`] replaces each with
//! `SelectionDAG::expandVAArg`: load the list pointer, round it up to the
//! type's ABI alignment when that exceeds the minimum stack argument
//! alignment (1, the pin sets none), step it by the type's allocation size,
//! store it back, and load the value from the rounded pointer. Sizes and
//! alignments are the target's fixed datalayout's.
//!
//! DEVIATE: MIR has no `va_arg` instruction, so the driver expands it in the
//! text, as arm64's does. A stopgap: the pin's MIR `va_arg` opcode replaces
//! both rewrites. Exit: 2026-11-20.
//!
//! # See also
//! `SelectionDAG::expandVAArg`, `SelectionDAGBuilder::visitVAArg`, pin
//! d1106deb71cc.

/// The ABI alignment and allocation size in bytes of the scalar `ty` under
/// m68k's datalayout (`i16:16:16-i32:16:32`, the defaults for the rest).
fn layout_of(ty: &str) -> Option<(u32, u32)> {
    Some(match ty {
        "i1" | "i8" => (1, 1),
        "i16" => (2, 2),
        "i32" | "ptr" => (2, 4),
        "i64" => (4, 8),
        "float" => (4, 4),
        "double" => (8, 8),
        _ => return None,
    })
}

/// `text` with each scalar `va_arg` of a `ptr` list expanded; a type this does
/// not know is left for the reader to refuse.
pub fn without_va_arg(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (number, line) in text.lines().enumerate() {
        let expanded = line
            .trim_start()
            .split_once(" = va_arg ptr ")
            .and_then(
                |(result, rest)| {
                    let (list, ty) = rest.split_once(", ")?;
                    let ty = ty.split(',').next()?.trim();
                    Some((result, list, ty, layout_of(ty)?))
                },
            );
        let Some((result, list, ty, (align, size))) = expanded else {
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let slot = format!("%va.{number}");
        out.push_str(&format!("  {slot}.list = load ptr, ptr {list}, align 2\n"));
        let at = if align > 1 {
            out.push_str(&format!("  {slot}.int = ptrtoint ptr {slot}.list to i32\n"));
            out.push_str(&format!("  {slot}.sum = add i32 {slot}.int, {}\n", align - 1));
            out.push_str(&format!("  {slot}.and = and i32 {slot}.sum, {}\n", -i64::from(align)));
            out.push_str(&format!("  {slot}.at = inttoptr i32 {slot}.and to ptr\n"));
            format!("{slot}.at")
        } else {
            format!("{slot}.list")
        };
        out.push_str(&format!("  {slot}.next = getelementptr inbounds i8, ptr {at}, i32 {size}\n"));
        out.push_str(&format!("  store ptr {slot}.next, ptr {list}, align 2\n"));
        out.push_str(&format!("  {result} = load {ty}, ptr {at}, align {align}\n"));
    }
    out
}
