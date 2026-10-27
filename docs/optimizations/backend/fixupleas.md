> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/fixupleas.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/fixupleas.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/fixupleas.rs sha256:45a630b57ae32d15bb961fd83f1f79933ea694031804964d97c4449f9cf3d5f9 -->

# fixupleas

`X86FixupLEAs`. Translated from
[X86FixupLEAs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L232)
(`runOnMachineFunction`), function by function, one PR each (plan:
`docs/working/lea-fixup-plan.md`).

## How it works
The pass visits every LEA of every block in order. On i486 none of
`slowLEA`, `slow3OpsLEA` or `leaUsesAG` is set, so only `optTwoAddrLEA`
(L564) runs on each: `opt_two_addr_lea`, its cases 1 and 2 and then
`optLEAALU` as case 3.

`optLEAALU` (L500, with `searchALUInst` L405) is `opt_lea_alu`: `lea
t,[x+y]` whose only reader, within five instructions, is `add d,t` or `sub
d,t` with the flags dead, is `op d,x` and `op d,y`. The adjacent case was
the peephole group `lea_alu`.

Done here: `is_lea` (L227), the block loop, `previous_instr` (L295),
`uses_register` (L270) and `search_backwards` (L308), which finds the
write of a register within five cycles before an instruction;
`inefficient_lea_base` (L331-347); `alu_names`, the pin's opcode maps
`getADDrrFromLEA` to `getINCDECFromLEA` (L351-400), which on our
name-keyed instructions are one table; and `check_reg_usage` (L466).

## Example
`a_write_beyond_five_cycles_is_not_found` in this file's tests: two loads
cost 8 cycles, so a write of the register before them is past
`INSTR_DISTANCE_THRESHOLD` and `search_backwards` finds none for the LEA
that reads it.

## Runs
After `FixupBW`, optimizing levels only.
