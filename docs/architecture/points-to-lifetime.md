---
architecture-index: points-to across edits
architecture-order: 80
---
# Points-to kept across edits

For contributors who want to reuse a points-to answer after a pass edits the
body. This is a design, not an implemented cache; the cost it targets is
tracked in [#779](https://github.com/ali-mosavian/llrm/issues/779) and
[#1166](https://github.com/ali-mosavian/llrm/issues/1166).

GCC computes points-to twice per function and keeps it across the passes
between; a new call resets to "anything". llrm solves for every body state and
its escape facts are positional (`escaped_before` per instruction), so a kept
answer must be repaired by what each edit did.

Today the value solve is already held per body (`PointerValues`); only the
escape phase is redone per configuration. See [modref.md](modref.md).

## What an edit must do

| Edit | Kept answer |
|---|---|
| erase / use of a subset | unchanged (a superset stays sound) |
| derived value (gep, cast) | union of operands'; a back-edge phi widens |
| new load of a pointer | escaped or stored-to objects, and Unknown |
| new alloca | new object, not escaped |
| stored pointer, new call | invalidate the body |
| copied block or insn | invalidate (no escaped_before entry) |
| moved insn | escape later or reader earlier: sound; otherwise invalidate |
| inline, signature change | invalidate (restrict roots and Parameter(i) renumber) |
| address rebuilt on a new base (lsr, gepoffset) | derive from the new base |
| callee declaration change | drops every body's answer in the module |

A missing `escaped_before` entry must read as `Unknown`, not as nothing
escaped. An `LLRM_CHECK_*` switch that asserts the kept answer is a superset of
a fresh solve is the instrument; `LLRM_CHECK_POINTVALUES` does this for the
held value solve.
