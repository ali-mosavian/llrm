//! A flag a compare or an arithmetic instruction sets is not clobbered before
//! the instruction that reads it.
//!
//! The pin's `MachineVerifier` checks that a live register's value reaches its
//! reader; the flags are a register there (`EFLAGS`) and the check covers them.
//! Here the flags are not allocated, and an instruction's expansion
//! decides what it writes (`inc` of `a` is `clc; adc #1` on the 6502: the carry
//! goes). So the check reads the expansion: for each block it follows the flags
//! the instructions marked `sets_flags` define, and an instruction that is not
//! one clears the flags its text writes. A reader (`reads_flags`) of a flag
//! cleared that way is an error.
//!
//! DEVIATE: no counterpart that reads the expanded text; the pin expands after
//! its verifier. Exit: the flags as a register of the shared allocator
//! (`D-6502`).
//!
//! # Runs
//! In `alloc::emit` when `LLRM_6502_CHECK_FLAGS` is set, on the text before the
//! peephole passes. A flag is checked within one block: a block's entry takes
//! what its predecessors leave.

use llrm_spike_mf::MFunction;

use crate::alloc::Class;
use crate::asm::{Line, Op};

const C: u8 = 1;
const Z: u8 = 2;
const N: u8 = 4;
const V: u8 = 8;
const ALL: u8 = C | Z | N | V;

fn writes(op: Op) -> u8 {
    match op.name() {
        "adc" | "sbc" => ALL,
        "cmp" | "cpx" | "cpy" | "asl" | "lsr" | "rol" | "ror" => C | Z | N,
        "sec" | "clc" => C,
        "clv" => V,
        "bit" => Z | N | V,
        "inc" | "dec" | "inx" | "iny" | "dex" | "dey" | "lda" | "ldx" | "ldy" | "pla" | "and" | "ora" | "eor"
        | "tax" | "tay" | "txa" | "tya" | "tsx" | "trb" | "tsb" | "plx" | "ply" => Z | N,
        "jsr" | "plp" | "rti" => ALL,
        _ => 0,
    }
}

fn reads(op: Op) -> u8 {
    match op.name() {
        "bcc" | "bcs" | "adc" | "sbc" | "rol" | "ror" => C,
        "beq" | "bne" => Z,
        "bmi" | "bpl" => N,
        "bvc" | "bvs" => V,
        _ => 0,
    }
}

/// What the lines read of the flags the instruction before them left, and every
/// flag they write.
fn effects(lines: &[Line]) -> (u8, u8) {
    let (mut read, mut written) = (0, 0);
    for line in lines {
        if let Line::Inst(inst) = line {
            read |= reads(inst.op) & !written;
            written |= writes(inst.op);
        }
    }
    (read, written)
}

fn letters(mask: u8) -> String {
    [(C, 'C'), (Z, 'Z'), (N, 'N'), (V, 'V')].iter().filter(|(bit, _)| mask & bit != 0).map(|(_, c)| *c).collect()
}

/// One instruction of a block: whether it sets and reads the flags, and the
/// text it expanded to.
pub struct Step<'a> {
    pub sets: bool,
    pub reads: bool,
    pub text: &'a [Line],
}

/// The error for the first reader in `steps` of a flag that an instruction
/// after the flag's setter wrote.
pub fn check_block(steps: &[Step]) -> Result<(), String> {
    let mut valid = ALL;
    let mut cleared_by: [Option<usize>; 4] = [None; 4];
    for (at, step) in steps.iter().enumerate() {
        let (read, written) = effects(step.text);
        if step.reads && read & !valid != 0 {
            let lost = read & !valid;
            let bit = lost.trailing_zeros() as usize;
            let by = cleared_by[bit].map_or(String::new(), |k| {
                let text: Vec<String> = steps[k].text.iter().map(|line| format!("{line:?}")).collect();
                format!(" by instruction {k} ({})", text.join("; "))
            });
            return Err(format!("instruction {at} reads {}, cleared{by}", letters(lost)));
        }
        if step.sets {
            // a compare of a byte with zero that expanded to nothing reads the
            // zero and sign the load or the `ora` before it left
            // (`alloc::emit`'s `sets_nz_of_a`)
            let kept = if written == 0 { Z | N } else { written };
            valid |= kept;
            for bit in 0..4 {
                if kept & (1 << bit) != 0 {
                    cleared_by[bit] = None;
                }
            }
        } else {
            valid &= !written;
            for bit in 0..4 {
                if written & (1 << bit) != 0 {
                    cleared_by[bit] = Some(at);
                }
            }
        }
    }
    Ok(())
}

/// [`check_block`] on every block of `function`; `starts[b][i]` is the first of
/// `lines` that instruction `i` of block `b` expanded to, and the entry after
/// the last is where the block ends.
pub fn verify(
    function: &MFunction<Class>,
    starts: &[Vec<usize>],
    lines: &[Line],
) -> Result<(), String> {
    for (b, block) in function.blocks.iter().enumerate() {
        let bounds = &starts[b];
        let steps: Vec<Step> = block
            .insts
            .iter()
            .enumerate()
            .map(|(i, inst)| Step {
                sets: inst.sets_flags(),
                reads: inst.reads_flags(),
                text: &lines[bounds[i]..bounds[i + 1]],
            })
            .collect();
        check_block(&steps).map_err(|why| format!("{} bb{}: {why}", function.name, block.id))?;
    }
    Ok(())
}
