> Generated from the `//!` headers of `crates/backend/llrm-core/src/backend`; edit the headers, not this file.

# Backend phases

From the `//!` headers of `crates/backend/llrm-core/src/backend`: 92 documented, 0 undocumented.

| Name | Summary | Runs | Page |
|---|---|---|---|
| addressforms | Port of `qbopt/backend/addressforms.py`: fold address arithmetic into the memory operands that read it. |  | [addressforms](addressforms.md) |
| addressvalues | Port of `qbopt/backend/addressvalues.py`: spell allocated address-valued memory operands as machine addresses. |  | [addressvalues](addressvalues.md) |
| affine | Allocated instructions that keep a register an affine sum of registers, and the 32-bit addresses that name such a sum. |  | [affine](affine.md) |
| allocate | Port of `qbopt/backend/allocate.py`: registers for a lowered body, using greedy allocation with eviction. |  | [allocate](allocate.md) |
| arithmetic | Port of `qbopt/backend/arithmetic.py`: target-dependent ranking of constant arithmetic, not MIR semantics. |  | [arithmetic](arithmetic.md) |
| arrival | Where a parameter is while the function has not yet stored it into its home: in the register the convention passed it in, from the entry until the instruction that stores it. |  | [arrival](arrival.md) |
| asm | The assembler: instructions in, one image and its relocations out. |  | [asm](asm.md) |
| assemble | A MIR module to masm, as LLVM's `llc` takes a module to an object: each defined function selected and run through the machine phases, each defined variable's data, and the symbols both name. |  | [assemble](assemble.md) |
| calleefacts | What a function nothing outside the program reaches leaves different: the registers its body, and the calls in it, write. |  | [calleefacts](calleefacts.md) |
| callregs | The registers a runtime call destroys or keeps, read from its contract. |  | [callregs](callregs.md) |
| cfi | Call frame information, read back from the code as it was emitted: from every place in a function, how to find the caller's frame (the canonical frame address) and the registers the function saved. |  | [cfi](cfi.md) |
| classes | What a target says about its registers that the allocator and the passes around it read: which operand of which instruction form lives in which register. |  | [classes](classes.md) |
| coalesce | Port of `qbopt/backend/coalesce.py`: a copy whose two values can share a register is not a copy. |  | [coalesce](coalesce.md) |
| comparefold | Port of `qbopt/backend/comparefold.py`: select one-use memory comparisons before register allocation. |  | [comparefold](comparefold.md) |
| constpool | Readonly constants the machine phases put in memory, as GCC's `force_const_mem`: one datum per distinct bytes, which the driver names and emits with the module's other readonly data. |  | [constpool](constpool.md) |
| constrain | Port of `qbopt/backend/constrain.py`: giving a required register a value of its own. |  | [constrain](constrain.md) |
| copyprop | Port of `qbopt/backend/copyprop.py`: forward and remove physical register copies proven on every CFG path. |  | [copyprop](copyprop.md) |
| copysink | Port of `qbopt/backend/copysink.py`: move a copy out of a loop when only what is after the loop reads it. |  | [copysink](copysink.md) |
| cpu | Port of `qbopt/backend/cpu.py`: one immutable description of a code-generation tuning target. |  | [cpu](cpu.md) |
| datagroup | The data segment register as a fourth selector register. |  | [datagroup](datagroup.md) |
| debuginfo | `-g`: a module's debug information as [`llrm_object::debug::Info`], whichever format a writer encodes it in. |  | [debuginfo](debuginfo.md) |
| division | Port of `qbopt/backend/division.py`: select signed constant division without exposing machine choices to MIR. |  | [division](division.md) |
| effects | The registers and flags an instruction reads and writes, as `x86.instr` states them: the rows the target's register file carries (`Info::effects`), for the code of `bits` bits. |  | [effects](effects.md) |
| ehprepare | A QB personality's landing pad made one the runtime can enter, before instruction selection, as LLVM's SjLjEHPrepare prepares setjmp/longjmp unwinding. |  | [ehprepare](ehprepare.md) |
| exactaddress | Fold the chain computing an exact address into its 32-bit form. |  | [exactaddress](exactaddress.md) |
| executed | A finished body's executed work, estimated without a profile. |  | [executed](executed.md) |
| farcall | Port of `qbopt/backend/farcall.py`: give a far indirect call the memory operand the 16-bit ISA requires. |  | [farcall](farcall.md) |
| farload | Port of `qbopt/backend/farload.py`: select complete far-pointer loads before allocation loses their address. |  | [farload](farload.md) |
| floatalloc | Port of `qbopt/backend/floatalloc.py`: assign floating LIR values to the target register stack. |  | [floatalloc](floatalloc.md) |
| floatassign | Pass one of GCC's x87 allocation, as IRA and LRA do for any register class: which floating values hold one of the eight flat stack registers, and where. |  | [floatassign](floatassign.md) |
| floatfold | A float load that only one arithmetic instruction reads is that instruction's memory operand: `fld [m]; faddp` is `fadd [m]`. |  | [floatfold](floatfold.md) |
| floatregions | What the x87 allocator raises, and where its stack cannot reach. |  | [floatregions](floatregions.md) |
| fpconvert | A float converted to an unsigned 64-bit integer, as signed conversions and a select, before instruction selection: the x87 stores only signed qwords. |  | [fpconvert](fpconvert.md) |
| frame | Port of `qbopt/backend/frame.py`: the stack slots a body needs, and where they are. |  | [frame](frame.md) |
| framefree | LLVM's `hasFP`, decided before allocation: whether a function can do without its frame register, which is then one more value register (and callee-saved: the prologue pushes it where it is used). |  | [framefree](framefree.md) |
| globals | A MIR module's globals as masm symbols and data, as LLVM's AsmPrinter emits a global's initializer: its bytes, and a relocation for each address in it. |  | [globals](globals.md) |
| inline_asm | Inline assembly: the x86 assembler is `llrm_x86::asm`; what is here is the QB runtime's naming of registers around it (`Reg`, the registers a runtime routine's contract names). |  | [inline_asm](inline_asm.md) |
| isel | Instruction selection from MIR: each function's instructions as LIR over virtual registers, as LLVM's SelectionDAG makes MachineInstrs. |  | [isel](isel.md) |
| jumps | Port of `qbopt/backend/jumps.py`: block order, and the jumps it makes redundant. |  | [jumps](jumps.md) |
| lanes | Python's `set[tuple[Register_, int]]` of register and flag lanes, as a bitmask. |  | [lanes](lanes.md) |
| layout | A whole body, emitted -- and everything that has to move when it does. |  | [layout](layout.md) |
| lifetimes | Where a block local's bytes are live, from its lifetime markers: from a `llvm.lifetime.start` along every path to where a `.end` or the return comes. |  | [lifetimes](lifetimes.md) |
| lirtext | LIR as text, one stage at a time: what `ISEL_DUMP` prints after each machine phase. |  | [lirtext](lirtext.md) |
| live | The live intervals the allocator holds of the body it is rewriting. |  | [live](live.md) |
| liveness | Port of `qbopt/backend/liveness.py`: which physical register and flag lanes are dead on exit from each block. |  | [liveness](liveness.md) |
| liveunion | What each register holds, by where: LLVM's `LiveIntervalUnion` and the `LiveRegMatrix` over it. |  | [liveunion](liveunion.md) |
| loopslots | A loop's spill slots in the registers the loop leaves free. |  | [loopslots](loopslots.md) |
| lower_int64 | The contract of an inline routine the compiler places where an operation has no instruction (`llrm_x86::helpers`): what it reads and what it changes. |  | [lower_int64](lower_int64.md) |
| machinecse | Port of `qbopt/backend/machinecse.py`: eliminate repeated deterministic computations after register allocation. |  | [machinecse](machinecse.md) |
| machinedce | Port of `qbopt/backend/machinedce.py`: eliminate pure allocated instructions whose complete result is dead. |  | [machinedce](machinedce.md) |
| masm | Allocated LIR as jwasm source. |  | [masm](masm.md) |
| nativeframe | Port of `qbopt/backend/nativeframe.py`. |  | [nativeframe](nativeframe.md) |
| nearcode | A function a frontend allows near (`nearcode`), only its own module calls, and only directly, is entered by a near call: its callers share its code segment, so a return address needs no segment. |  | [nearcode](nearcode.md) |
| needs | What a body contains that a peephole sub-pass could act on, so that the driver skips a pass whose subject is absent: the shape of the MIR passes' `Depends`, for the machine passes. |  | [needs](needs.md) |
| neighbours | The values one value interferes with: a sorted set that is a bit vector once it is dense, as GCC's IRA keeps the conflicts of an allocno (a vector of ids, or a bit vector over its id range, whichever is smaller: `ira_allocate_object_conflicts`, ira-build.cc). |  | [neighbours](neighbours.md) |
| objbuild | A masm module as an object: its code and data laid out in sections, each name resolved to where it is defined or declared, each reference to one a relocation. |  | [objbuild](objbuild.md) |
| overlap | Whether two machine memory operands may overlap, read from their addresses alone: the same space and segment compared by offset and width, and "may overlap" for anything else. |  | [overlap](overlap.md) |
| parcopy | Port of `qbopt/backend/parcopy.py`: scheduling a parallel copy, moves that happen at once, written in an order. |  | [parcopy](parcopy.md) |
| peephole | Port of `qbopt/backend/peephole.py`: simplifications that depend on the final physical register assignment. |  | [peephole](peephole.md) |
| phielim | Port of `qbopt/backend/phielim.py`: the pass that takes LIR out of SSA. |  | [phielim](phielim.md) |
| pointers | Port of `qbopt/backend/pointers.py`: target pointer representation, kept below the MIR boundary. |  | [pointers](pointers.md) |
| postings | Where each value is defined and read in a body, kept as the body changes. |  | [postings](postings.md) |
| postrasink | LLVM's PostRAMachineSinking: a copy whose result only one successor reads moves into that successor. |  | [postrasink](postrasink.md) |
| pressuresink | A load made where its one reader is, when that raises the pressure of no register class: LLVM's scheduler works bottom-up for the same reason, a value defined late is live a short while. |  | [pressuresink](pressuresink.md) |
| prologue | Port of `qbopt/backend/prologue.py`: reserving the frame the spiller asked for, and giving it back. |  | [prologue](prologue.md) |
| regalloc_input | Test support: a function of a MIR fixture as the register allocator receives it, and the allocator phase itself. |  | [regalloc_input](regalloc_input.md) |
| regclass | The register class of each value: the one place that states which registers a value may be in. |  | [regclass](regclass.md) |
| registerinfo | A target's register file, as its description states it (`registers.regs`) and the target generated it: the `Info` a `Target` hands out and every query of a register asks. |  | [registerinfo](registerinfo.md) |
| regthrash | Port of `qbopt/backend/regthrash.py`: renaming a definition's register so a copy after it is redundant. |  | [regthrash](regthrash.md) |
| relayout | The frame laid out again: a function whose frame reaches past `[bp-128]` is also tried with a hole above its allocas for the spill slots; this makes that layout from the first one, by the slot each frame cell names, where it used to run the whole backend a second time (docs/optimizations/second-frame.md). |  | [relayout](relayout.md) |
| rmw | Port of `qbopt/backend/rmw.py`: select exact read-modify-write chains before register allocation. |  | [rmw](rmw.md) |
| schedule | Port of `qbopt/backend/schedule.py`: latency-aware ordering of safe, already-allocated machine instructions. |  | [schedule](schedule.md) |
| selects | Selects as branches: the 486 has no conditional move, so each `select c, a, b` becomes a branch on `c` around an empty block into a phi of `a` and `b`, before instruction selection. |  | [selects](selects.md) |
| sharedstores | Tuned for size, a run of stores of one literal is the literal in a dead register, loaded once, then register stores: `mov dword ptr [bp-8],0` is 8 bytes, `mov [bp-8],eax` 4, and the one `xor eax,eax` 3. |  | [sharedstores](sharedstores.md) |
| shrinkwrap | LLVM's ShrinkWrap: the registers a procedure keeps for its caller (SI and DI), and its frame (the stack reserve and the frame register), set up where they are first needed instead of at every entry, and taken back only on the returns that path reaches. |  | [shrinkwrap](shrinkwrap.md) |
| slots | The one rule for sharing a frame slot: two things share when they are never live together. |  | [slots](slots.md) |
| spiller | Port of `qbopt/backend/spiller.py`: a value the allocator would not keep, kept in memory instead. |  | [spiller](spiller.md) |
| spillforward | Port of `qbopt/backend/spillforward.py`: remove reloads of a slot a register already holds on every path here, and stores of such a register back into that slot. |  | [spillforward](spillforward.md) |
| spillplacement | Where a split value should be in a register and where on the stack. |  | [spillplacement](spillplacement.md) |
| splitkit | Port of `qbopt/backend/splitkit.py`: splitting a live range instead of spilling the whole of it. |  | [splitkit](splitkit.md) |
| ssarepair | SSA after a spiller gave some values more than one definition: a reload brings a value back, and every use after it must read the reload, or a phi where two definitions meet. |  | [ssarepair](ssarepair.md) |
| ssaspill | Braun & Hack spilling on the SSA LIR ("Register Spilling and Live-Range Splitting for SSA-Form Programs", CC 2009), run before `PhiElimination`. |  | [ssaspill](ssaspill.md) |
| stackusage | How much stack a program may use, as GCC's `-fstack-usage` and `-Wstack-usage`: each procedure's own frame, and the most a call from it can reach through the procedures it calls. |  | [stackusage](stackusage.md) |
| stepflags | A compare of a value with a constant that a step of the same value already subtracts: the step's flags are the compare's. |  | [stepflags](stepflags.md) |
| storecombine | Port of `qbopt/backend/storecombine.py`: pack neighboring literal word stores after allocation. |  | [storecombine](storecombine.md) |
| storedhomes | The cells the optimizer proved hold a phi's value (`!llrm.home`, `LirBody.homes`), as the body now stands: the one fact both spillers ask. |  | [storedhomes](storedhomes.md) |
| target | Port of `qbopt/backend/target.py`: what the machine has, and what each instruction requires. |  | [target](target.md) |
| timing | Port of `qbopt/backend/timing.py`: audited instruction core-clock bounds, separate from scoreboard estimates. |  | [timing](timing.md) |
| twoaddr | Port of `qbopt/backend/twoaddr.py`: x86 writes into one of the registers it reads. |  | [twoaddr](twoaddr.md) |
| upperzero | Which 32-bit register roots hold zero in their upper half before each allocated instruction. |  | [upperzero](upperzero.md) |
| valuetrack | Where the value a variable has is, over the code as it was emitted: LLVM's instruction-referencing LiveDebugValues (`InstrRefBasedLDV`), after the allocator and every pass that follows it. |  | [valuetrack](valuetrack.md) |
| verify | Port of `qbopt/backend/verify.py`: whether a lowered body is well formed, checked where it was made. |  | [verify](verify.md) |
