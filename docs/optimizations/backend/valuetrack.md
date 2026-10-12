> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/valuetrack.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/valuetrack.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/valuetrack.rs sha256:60508766cf2aefdd91b42335c4e111e3bc65651da9bd3498d942d2cf5fecf555 -->

# valuetrack

Where the value a variable has is, over the code as it was emitted: LLVM's
instruction-referencing LiveDebugValues (`InstrRefBasedLDV`), after the
allocator and every pass that follows it.

Instruction selection tells which instructions define a value `-g` names,
and masm marks where each landed (`Mark::Def`: the register or frame cell
the instruction before the mark wrote) and where a variable takes a value
(`Mark::Note`). Nothing else of the allocator is read. The code is decoded
and followed from its entry along every path, with what each register and
frame cell holds: a move copies it, any other write loses it, a call loses
what the callee clobbers, and where paths join only what all of them agree
on is kept. A variable is where its value is: a register if one holds it,
else a cell.

No value is the answer wherever the code cannot be followed or the value is
in no register or cell. An optimizer that wrote the value somewhere
unrecorded costs the variable its location there, never a wrong one.
