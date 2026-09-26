# Fragility

What has broken, or let a break go unseen, and why. Each entry says what guards it now.

## Measurement

**A hung demo reported the last run's results.** From df89fc8a on, qbdemo hung in DOSBox, and the demo runner scored the previous run's `P*.BIN` files for a day: every "all marks same" and screenshot sheet in that time was stale. The runner waited for its done-marker, timed out silently, and read what was on disk. Guard: the runner deletes old results first and fails when DOSBox does not finish. Identical timings to 0.1 ms after a code change are the tell.

**DOSBox cannot see memory operands.** Its fixed-cycle core charges per instruction, so turning `add ax,[bp-6Ah]` into `add ax,bp` leaves the demo times unchanged. Unchanged time is not evidence of no gain, and a gain in time is not evidence of fewer memory operands. Judge such changes by `tools/innerloops.py` and the 486 cost table.

**The loop scorer decoded data as code.** `tools/innerloops.py` decoded the segment from byte 0, so the module header's bytes hid the `stride` loop. Guard: it decodes only what control reaches from the entries (05116418).

**A regression test passed at HEAD.** The first count-to-zero test (RIPPLE) never reproduced the bug; the loop was already right in isolation, and went wrong only beside another loop. A test not seen to fail is evidence of nothing; the replacement was checked against the HEAD build before and after.

**One price for two encodings.** The scorer and scheduler priced `shl r,1` (the D1 form, three clocks on the 386 and 486) as the two-clock imm8 form, so nothing saw that `add r,r` doubles in one. A golden carried over from the Python scorer held the wrong price as the expected answer. Guard: the D1 form has its own row, with a test.

**The first test shape never reached the pass.** A one-dimensional lookup put its doubling into a scaled 32-bit address, so the new test passed without the fix. Check the listing contains the instruction under test before trusting the assertion.

**The allocator prices spills the later passes undo.** `_traffic` charges a memory operand per reference of a spilled value, but spillforward and loopslots then delete many of those reloads. Every accept-or-reject gate built on it (region split, trials) judges against a cost the output does not pay: PARTICLE's selector was priced at 117 and cost 0 after loopslots. Guard: none yet. The spiller must own reload placement so the gate asks the same fact.

**Batch acceptance hides good splits behind bad ones.** A round's splits are carved together and kept only if total traffic falls. Adding single-instruction pieces changed the plan, so CYCLEBLOBS' region split went from accepted to rejected (21765 → 24804) and took every good split with it. Guard: a piece is carved only if its references outweigh its copies, or if the rest of the value got a register. Single-instruction pieces are gone until they are priced.

## Contracts

**A call's declared effect read only its arguments.** The callee also runs on the caller's BP chain, SP, DS, SS and CS. Once liveness took a call's effect from that contract (df89fc8a), the peephole removed `mov ds, ss` before a call as dead, and qbdemo hung in `UPDPALPLASMA`. The old code treated an undecoded call as reading everything, which hid the gap. Guard: a call reads the same state a return does (8971698d), with a test.

**Refactoring one fact into one place changed its answer.** df89fc8a made liveness, regthrash and machinedce ask one `effect` per instruction — the right shape — but the shared answer carried the narrowest caller's assumption about calls. When facts merge, check each former caller's fallback, not only the common case.

**A parameter nobody read.** `ranges::constants` took the data group, but `consts::known` reads it only when calls are also given, and they never were. Hoist passing `None` and loop motion passing the group got the same answer, which the plan took for two derivations of one fact. Guard: the parameter is gone from `constants` and the functions that only forwarded it.

## Pass order and interaction

**A pass that settles early decides on another pass's leftovers.** Count-to-zero ran inside strength reduction, each round, so it rotated a loop before strength had made that loop's pointer, and chose the counter a later round would have killed. Guard: passes run in stages; count-to-zero waits until strength reduction has settled.

**A reorder hid a bug.** Before df89fc8a, running `fused` before `high_extracts` hid SHLD's false read of its destination. A pass order that makes something work is a fact nobody wrote down; the fix was to make the effect right.

**A credit that ignores what follows.** Strength reduction freed a counter whenever its scaled uses covered it, even with a symbolic trip count. Count-to-zero then had to take control through a pointer at a symbolic bias, which cost a register in every address and spilled (SUMTHREE 8 → 9). Guard: the credit applies only when a root can take control at a constant bias.

## Invariants of the machine form

**Value ids outlive the instructions that define them.** Deleting a reload in LIR left its readers naming the reload's value, and the verifier rejected the body. Registers are allocated, but `uses`, `requires` and segment selectors in memory operands still name values. Removing a definition means renaming what reads it.

**A home must be an operand the instruction takes.** Loopslots swapped a slot for a register in every instruction that touched it, including `fistp`, which stores only to memory. No loop reached that case until parking freed a register in one, and the object writer then failed on `fistp ax`. Guard: a slot takes a register only if every instruction touching it still encodes, with a test.

**BP is the runtime's frame chain.** Error handling walks it; a loop may hold a value in BP only when it cannot call, trap or touch x87 state, and only between `push bp` and `pop bp` on every entry and exit edge.

**DS is a register the program model reserves only at calls.** Between the points that need the data group, the allocator may give DS an array's selector, so every call site must restore it; nothing but the call contract enforces that.

**A split made a remakeable value a spill.** The piece took the `lea` and the rest was defined by a copy back, so the spiller no longer saw an address to remake, and a frame address crossing calls got a slot. Guard: a split point remakes a value whose only definition reads nothing (LLVM's `defFromParent`), with a test on the frame size.

**Planned positions go stale.** Regions name instruction positions, and carving one split inserts copies that shift them for the next. Guard: `carved_moving` returns where each position went, and later regions are moved through it.

**A single-block loop shares a bundle with its preheader and exit.** A value whose register is taken for the whole of the block before a loop can never be placed in the loop. The border is only open when interference ends before the preheader's terminator, as LLVM's `addSplitConstraints` requires.

## Build and harness

**The sync script ships only tracked changes.** A new file must be `git add -N`'d before a remote build sees it.

**Stage lists are asserted.** Adding a machine pass changes the observer test's expected stage names.

**The build host is shared.** Its disk filled transiently under another session's load, failing a build and a compile with "No space left on device".

**Static data over 64 KB panics the object writer** instead of reporting a compile error (queued separately).
