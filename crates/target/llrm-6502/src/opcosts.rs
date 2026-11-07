//! What each operation class costs on the 6502, in clocks and in code bytes,
//! read off the sequences the backend emits for it.
//!
//! The mid-end prices a body by [`OperationCosts`]. The machine used to answer
//! with the all-ones default, so an `i32` add cost what an `i8` add did and a
//! 220-byte fill looked like a few instructions. These tables replace it: one
//! canonical snippet per class, priced by [`timing::estimate`] (clocks and
//! bytes of the same text), per native byte for the classes that scale with
//! the operand's width (`OperationCosts::native_bits` = 8).
//!
//! DEVIATE (6502-OPCOSTS): the pin prices by its own target's scheduling model;
//! the 6502 has none, so the tables are measured here. Exit: a generated
//! scheduling description of the instruction tables (2026-11-20).
//!
//! # Example
//! ```text
//! add   clc; lda rc2; adc rc3; sta rc2   -> 7 B, 11 clk per byte of the operand
//! load  ldy #0; lda (rc0),y; sta rc2     -> 6 B, 12 clk per byte
//! ```

use llrm_mir::target::OperationCosts;

use crate::dagisel::hooks::Cpu;
use crate::timing;

/// The class of an operation and the text it is priced by. `per_byte` classes
/// are multiplied by the operand's byte count in `profit::operation`.
pub const SNIPPETS: &[(&str, &str)] = &[
    ("add", "clc\n  lda rc2\n  adc rc3\n  sta rc2\n"),
    ("load", "ldy #0\n  lda (rc0),y\n  sta rc2\n"),
    ("store", "lda rc2\n  ldy #0\n  sta (rc0),y\n"),
    ("move", "lda rc2\n  sta rc3\n"),
    ("shift", "asl rc2\n"),
    ("branch", "lda rc2\n  cmp rc3\n  bne l\nl:\n"),
    ("address", "clc\n  lda rc2\n  adc #1\n  sta rc2\n  lda rc3\n  adc #0\n  sta rc3\n"),
    ("call", ".import callee\n  jsr callee\n"),
    ("return", "rts\n"),
];

/// Clocks and bytes of the snippet of `class`.
pub fn snippet(
    cpu: Cpu,
    class: &str,
) -> (i64, i64) {
    let (_, text) = SNIPPETS.iter().find(|(name, _)| *name == class).expect("a class of the table");
    let priced = timing::estimate(cpu, text).expect("a snippet the estimator prices");
    (priced.cycles as i64, i64::from(priced.bytes))
}

/// The class prices, `pick` choosing clocks (`.0`) or bytes (`.1`).
fn table(
    cpu: Cpu,
    pick: fn((i64, i64)) -> i64,
) -> OperationCosts {
    let at = |class| pick(snippet(cpu, class));
    OperationCosts {
        native_bits: 8,
        add: at("add"),
        load: at("load"),
        store: at("store"),
        r#move: at("move"),
        shift: at("shift"),
        branch: at("branch"),
        address: at("address"),
        call: at("call"),
        return_: at("return"),
        ..OperationCosts::default()
    }
}

/// Prices in clocks.
pub fn clocks(cpu: Cpu) -> OperationCosts {
    table(cpu, |(clocks, _)| clocks)
}

/// Prices in code bytes.
pub fn bytes(cpu: Cpu) -> OperationCosts {
    table(cpu, |(_, bytes)| bytes)
}
