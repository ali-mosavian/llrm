# The second frame: lay the frame out again, not run the backend again

Status: step 1 is in (#919); step 2 is #930 (`backend/relayout.rs`, `select::priced_in`, `LLRM_CHECK_FRAME`): the second layout is made from the first run's frame; every pricing and encodability call prices a frame cell as near, since a price that knew a displacement made the two layouts decide differently on m32 (sieve `-m32 -O2`: `add [slot], 1` against reload, add, store). The check finds no difference on bench m32 and tests/run at `-O0/-O2/-Os`, QCport m16 at `-O0/-O2/-Os` and `-g`, and the workspace tests. Step 3, moving the readers from displacements to slot ids, is open.

TL;DR: for a function whose frame has operands past `[bp-128]`, `machined_once` runs the whole backend a second time with a hole above the allocas for spill slots, and keeps the run with fewer far operands. The second body equals the first with its frame displacements changed in 270 of 270 runs on QCport (m16, `-O0`, `-O2`, `-Os`), and the run costs 24.5% (`-O0`) to 35.8% (`-O2`) of the backend's time there. Proposal: compute the second layout from the first run's frame and body, keep the old second run as the reference under `LLRM_CHECK_FRAME`.

## What happens today

`assemble.rs` `machined_once`: run 1 with `hole = 0`; if `spilled = frame.floor + reserve > 0`, the frame has a floor and `far_frame(first) != 0`, run 2 with `hole = spilled`; keep run 2 if `far_frame(second) < far_frame(first)`. Each run is `cheaper`: the spiller candidate, and where it changed something the allocator-alone candidate. isel starts its stack depth at `hole` (`isel.rs` 733, 869-870, 4120-4122), so every cell isel makes (allocas, temporaries) lies `hole` bytes lower; `Frame::slot` (`frame.rs` 123-135) hands spill slots out of `[-hole, 0)` first, top down, and below the lowest home after that.

## Measured

Instrumented `machined_once` to compare the two bodies instruction by instruction with every BP-relative displacement zeroed (`Mem.addr.disp`, `offset`, `disp_width`, for frame cells and for SS-segment cells indexed through a register): QCport's 65 modules, 270 functions with a second run: identical in all 270 (56 at `-O0`, 108 at `-O2`, 106 at `-Os`). A first version of the comparison that skipped indexed cells reported 68% and was wrong; the 32% it called different were array cells whose displacement moved with the allocas.

Share of the backend (`assemble`) spent in the second run: QCport `-O0` 24.5%, `-O2` 35.8% (16 and 26 of 65 modules have one). The m32 bench kernels have almost none (their frames are small).

## Proposal: frame indices, as LLVM has them

A displacement computed by a replay is a patch over a decision made too early: cells name BP offsets from isel on, and anything that reads an offset before the frame is final (an encoding size, an adjacency, a debug home, bytes patched into inline code) must be remade when the layout moves. The general form: a frame cell names a slot (a frame index: slot id, offset inside the slot, kind) until one frame-finalization pass after allocation and the peephole assigns the offsets, so that `far_frame` is an input to that pass and the second run disappears with nothing to keep honest.

### Who reads a concrete offset today (read-only survey of `llrm-core/src` and the x86 encoder, spot-checked at isel.rs 3183 and 2631, select.rs 209-232)

| class | where | with a slot id |
|---|---|---|
| layout (decides the numbers) | isel allocas, temporaries, homed arguments (860-976, 4162); `Frame::slot`/`cell`/`of` (frame.rs); spiller slot colouring (`siblings`, `_color_slots`, `_existing_colors_by`); `prologue`, `masm` reserve, `driver/basic.rs` `_static_frame`/`_runtime_frame` | one place owns ordering, hole, floor, reserve: the finalization pass |
| identity / overlap | `overlap.rs` `frame_bytes`; `peephole` `_frame_cell`; `spillforward`; spiller `_keeps`/`_may_write`/`_identity`/`_copied_with`; `loopslots`; `CallMemory.private` (built as the complement of escaped byte ranges) | `slot_a == slot_b` and offsets meet; `private` becomes a set of escaped ids with a default-private rule |
| adjacency arithmetic | `peep/guards.rs` `next_word`, `farload.rs`, `peephole.rs:733`, spiller `_address_source`, `exactaddress.rs`, isel `Pointer::moved` | `(slot, off + 2)`; a merge of two slots is refused (one object is one slot) |
| cost or encoding by value | x86 `select.rs` 209-232, 280-294 (a Frame operand's size is derived from `disp`, `disp_width` ignored); `assemble.rs` `cost` (`-Os` sums encoded bytes), `far_frame`; `ssaspill` `reload_price`; peephole byte comparisons; `sharedstores`; `jumps` `_arm_bytes` | a size estimate by kind; this is where objects can change (below) |
| alignment | `WORD` rounding in frame.rs, isel 1359, 4163, masm 363 | kind carries it |
| debug | isel 1818 builds `DebugPlace::At(frame disp)`; `_static_frame`/`_runtime_frame` rewrite it; `debuginfo.rs` | `(slot, off)`, resolved by the pass as `_static_frame` already does; `arrival.rs` reads bytes and is unaffected |
| sign tests for incoming arguments | `disp > 0`, `>= 4`, `spills_at` | `kind == Incoming`, `kind == Spill` |

Cannot be symbolic, by their nature: (1) the inline-code bytes of isel 3183, which patch `disp + addend` into a byte vector (needs fixup records resolved by the pass); (2) fixed cells from native or lifted input, the incoming arguments, `variadic`, the interrupt save area (fixed frame indices with an immutable offset; `floor` is the end of that region); (3) indexed arrays, which are `Addr{Literal, disp, SS}` through BP and carry no slot today (need a slot id on that form); (4) the frame-size immediates (`prologue._adjust`, `_arguments`) and the `far_frame` decision itself, which needs a tentative layout.

### Order of steps

1. Annotation, no behaviour change. Every frame cell gets a slot id and an offset inside its slot alongside the concrete displacement it has now (`Addr.index` is unused for Frame cells; the indexed form needs a field). isel, `Frame::slot`, the spiller's colouring and the inline-code patches record it; arithmetic on a cell keeps the id. A verifier rule fails the compile on a frame cell without one: that is the check that nothing was missed, and it is a cheap one. Decisions still read the concrete numbers of layout A, exactly run 1's.
2. The finalization pass: take the layout (A, or B with the hole), assign offsets from `(slot, off)`, rewrite cells, debug homes, inline-code fixups and the reserve; choose A or B by `far_frame` over the result. The second backend run is deleted.
3. Optional, later, one reader at a time: migrate the identity/adjacency readers to slot ids, then the cost readers to size estimates. Each is its own change; none is needed for the second run to go.

### Do objects change

After step 2 an object differs from today's only where run 2 would have decided differently from run 1 on a displacement-dependent choice (the candidate `cost` at `-Os`, `jumps`, peephole byte tests): none of 270 did. Because step 2 rewrites the whole frame from slot ids, it also fixes what a displacement remap would have missed (debug homes, inline bytes); step 3 is the one that may change objects, and is not proposed here. If the corpus shows a difference, the gate shows it as a changed object, not a silent one.

### Size

Step 1 touches about fifteen files (isel, frame, spiller, ssaspill, floatassign, floatalloc, farcall, loopslots, driver/basic, the omf `Addr` form and its printer, the verifier) and is the bulk: an estimate of 600-900 lines and a new test per producer. Step 2 is about 150 lines. A displacement remap by replay would be about 150 lines in total and is what this replaces; it is cheaper and has the weakness above.

## Not done

No predictor of which run wins: the rule stays `far_frame`. The two candidates inside a run (spiller / allocator alone) are a separate question (a near coin flip per function, measured separately).

Expected: the second run's 24-36% of the backend on programs that have one.
