# Rematerialization, in-place reads and forwarding

Audience: an engineer asking why a spilled value has no store, or why a reload is gone. This page states which values are made again instead of stored, and which reloads the later passes remove.

## Made again

For a value the allocator spills, `spiller::planned` first looks for a way to produce it where it is read, with no slot ([spilling](spilling.md)). Each needs the producer to be safe to repeat:

| Value | Made again as | Condition |
|---|---|---|
| a constant | `mov` of the immediate, at the width of the read | one definition, or definitions that are alike (`_literals_by`) |
| a frame or global address | `lea` of the same address, or folded into the one memory operand that uses it | a pure address (`_addresses_by`) |
| an extension of a value | the same extension at its use | one unchanged, ordinary use ([`test_single_use_extension_is_rematerialized_at_its_use`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/spiller.rs#L5035)); not from the entry of a loop |
| a stable load | a load from the same cell | nothing may write the cell while the value lives (`_stable_loads`, with `overlap::may_overlap`); a copy of the load is made again as the load, through `_through_copies` |
| a phi value the optimizer proved is also in a cell | a load of that cell | `storedhomes::held`: the cell is not written while the value lives and every definition is a plain copy |

A copy of a value that is made again is made again the same way. A made-again value is marked `rematerialized`; the dump shows it as a plain `lea` or `mov`, and `executed` counts it as a remat. Tests: [`test_spilled_constant_is_rematerialized_without_a_frame_slot`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/spiller.rs#L4799), [`test_spilled_frame_address_is_rematerialized_without_a_frame_slot`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/spiller.rs#L4937), [`test_a_copy_of_a_load_is_not_made_again_after_its_cell_changes`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/spiller.rs#L5434). In [examples](examples.md#a-loop-with-more-values-than-registers) the loop's `lea` of a frame address is one.

## Reads in place

Before any candidate runs, `RegAlloc::transform` removes the values for which holding a register never pays: `spiller::_folded_reads` finds a value loaded from a cell that holds, read once, by an instruction that takes the cell as its memory operand (or a `push`), that runs no more often than the load. Reading the cell there costs the same one memory operand and no instruction, and no register is held between. The value is spilled up front, outside the search.

An instruction that reads a spilled value takes the slot directly where its form allows it: `add`, `sub`, `and`, `or`, `xor`, `cmp`, a 32-bit `imul` ([`_foldable`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/allocate.rs#L3811), `spiller::folded_source`). The weight of a value is lowered by what such folds save, by the CPU's prices of the register and memory forms.

## Forwarding

After allocation the peephole runs [`spillforward::forwarded`](https://github.com/ali-mosavian/llrm/blob/82848d3b75a0dc0b0541d99bbcc5071361710eb9/crates/backend/llrm-core/src/backend/spillforward.rs#L261): a register holds the bytes of every slot it has been read out of or written into since; across blocks the fact must hold on every incoming edge, an availability problem solved over the body. A reload of a slot a register already holds is removed, and so is a store of a register back into the slot it holds. Any instruction whose register effects cannot be read, or that writes memory the displacement alone does not name, ends every fact.

`postrasink::sunk` then moves a copy or spill store into the one successor that reads it, and `regthrash` renames a definition's register so a following copy is redundant, which removes copies the coalescer refused on colourability ([coalescing-and-copies](coalescing-and-copies.md#after-allocation)). Loop-level traffic is in [after-allocation](after-allocation.md#spill-traffic).
