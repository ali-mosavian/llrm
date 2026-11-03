//! `StackColoring`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/StackColoring.cpp#L1)):
//! the allocas never live together share one frame object. The grouping is the
//! shared backend's (`llrm_codegen::backend::stackcoloring`, DEVIATE SC1:
//! computed on the IR's lifetime markers, exit 2026-11-20 with the MIR-level
//! pass); like the pin it is applied after selection, so the DAG still tells
//! the allocas apart.

use crate::mir::{Function, Operand};

/// Frame index `i` of the first `colors.len()` objects becomes `colors[i]`,
/// each object the largest of its group; the later objects follow them.
pub fn stack_coloring(
    f: &mut Function,
    colors: &[i32],
) {
    let groups = colors.iter().map(|&color| color as usize + 1).max().unwrap_or(0);
    if groups == colors.len() {
        return;
    }
    let mut frame = vec![(0, 1); groups];
    for (&(size, align), &color) in f.frame.iter().zip(colors) {
        let object: &mut (u64, u64) = &mut frame[color as usize];
        *object = (object.0.max(size), object.1.max(align));
    }
    frame.extend_from_slice(&f.frame[colors.len()..]);
    f.frame = frame;
    let moved = |index: i32| match colors.get(index as usize) {
        Some(&color) => color,
        None => index - colors.len() as i32 + groups as i32,
    };
    for inst in f.blocks.iter_mut().flatten() {
        for operand in &mut inst.operands {
            if let Operand::Frame(index) = operand {
                *index = moved(*index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dagisel::generated::ids::opcode as op;
    use crate::mir::Inst;

    /// Two allocas never live together kept an object each: jpeg's frame was
    /// 400 B where the pin's is 336, and every offset past the first took a
    /// second LEB byte.
    #[test]
    fn allocas_in_one_group_are_one_object() {
        let mut f = Function::default();
        f.frame = vec![(64, 4), (48, 8), (16, 4)];
        f.blocks = vec![vec![
            Inst::new(op::BR, &[Operand::Frame(0)]),
            Inst::new(op::BR, &[Operand::Frame(1)]),
            Inst::new(op::BR, &[Operand::Frame(2)]),
        ]];
        stack_coloring(&mut f, &[0, 0]);
        assert_eq!(f.frame, [(64, 8), (16, 4)]);
        let frames: Vec<_> = f.blocks[0].iter().map(|i| i.operands[0]).collect();
        assert_eq!(frames, [Operand::Frame(0), Operand::Frame(0), Operand::Frame(1)]);
    }
}
