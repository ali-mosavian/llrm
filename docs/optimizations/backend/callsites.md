> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/callsites.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/callsites.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/callsites.rs sha256:2e0e09001830d902f7d1dd407194395cdacf362ccd1078ace695fbd757ddd4b6 -->

# callsites

The calls of emitted code and what their argument registers hold
(`DW_TAG_call_site`).

## How it works
The emitted bytes are decoded. Each `call` is a site; its callee is the
symbol a fixup names in the instruction. For each argument register the
callee takes, the straight run of instructions before the call is read
backwards to the first that writes it:

- `mov reg, imm` or `xor reg, reg`: a constant;
- `mov reg, saved` or `lea reg, [saved+d]`, `saved` a register calls
  preserve and no later instruction writes: that register plus `d`;
- none, back to the entry: the value the register had at entry.

A branch, a call or a branch target ends the run, and the register is then
left out: a site that says less is true, one that says more may not be.

## Example
```text
 mov esi, 7        ; rsi = 7
 mov rdi, rbx      ; rdi = rbx + 0
 call g            ; site: g, return pc, rsi=7, rdi=rbx
```

## Runs
On the emitted code of a procedure, from `debuginfo`, for targets whose
description names argument registers.
