---
architecture-index: arm64 isel and peephole gaps against llc, as the table patterns to write
---

# arm64 selection patterns to write

TL;DR: the gaps below are measured against the pinned `llc -O2` on clang's `-O2` IR; each becomes a `.isel` or `.peep` pattern when the generators take an arm64 `.instr` (backend-two PRs 10-23 operand kinds, 10-27 flags). Until then the hand selector in `crate::isel` stands in, and each pattern's test is the oracle for the code it replaces.

Executed instructions against llc: 1.13x on `tests/programs`, 1.21x on `tests/kernels`, 1.63x on the conditions kernel (`tools/targets/arm64_attribute.py`, `tools/targets/arm64_diff.py`).

| Gap | MIR shape | arm64 form | Measured | Test |
| --- | --- | --- | --- | --- |
| Indexed access | `load(gep(global/slot/reg, idx))` | `ldr rt, [rn, rm{, ext #s}]` | done by hand (`global_index`) | `a_global_array_is_indexed_in_the_access` |
| Wide constants | `const` of 2+ `mov` words | one definition, expanded late | done by hand | `a_wide_constant_is_built_outside_the_loop...` |
| Multiply-add | `add(mul(a, b), c)`, `sub(c, mul(a, b))` | `madd`, `msub` | `madd` -3 | to write |
| Conditional compare | `and`/`or` of two compares feeding a branch or select | `ccmp`, `ccmn` | `ccmp` -4 (kernels) | conditions kernel |
| Conditional increment | `select(c, x + 1, x)` | `csinc`, `cinc` | `cinc` -2 | to write |
| Float negate-multiply | `fneg(fmul(a, b))`, `fsub(0, fmul)` | `fnmul`, `fnmadd`, `fnmsub` | `fnmul` -3 | to write |
| Float constants | `fconst` not `fmov`-encodable | constant pool, `adrp`+`ldr` | `movk` +7 | to write |
| Shifted/extended operand | `op(x, shl(y, k))`, `op(x, sext(y))` | shifted/extended register forms | `add` +40 | to write |
| Pre/post-index | `load(p)`, `p + k` in a loop | `ldr rt, [rn], #k` | needs LSR | after LSR |

What the generators must express (sent to backend-two): shift kind and amount, extend kind and amount, and condition as operand kinds; `when PRED` on a bound immediate (logical-immediate, shifted 12-bit, bitfield mask); two fused inputs in one pattern (`ccmp`); a `csel`-family result (`Operation::Select`).

`llrm_target::instr` still names x86 in `REGISTERS`, `CONDITIONS`, the flag table, the `iced` column and the error strings; the arm64 rows wait for those to come from the target.
