# Register classes on the body (the pin's MRI shape)

Owner: splitkit-port. Status: parked, numbered row; plan only.

TL;DR: the pin reads each vreg's class (`MRI::getRegClass`); here `regclass::collected` rescans the body's reads for it: 46 scans in a d_faces -O2 compile, `collected` 7.0 M Ir, `constrained` 2.2 M, `required` 1.0 M (about 10 M of the +70 M regalloc excess; the pin has no counterpart).

## Shape

- Each value gets a base class when it is made (its type's class: byte, word, dword, x87) and a pass that builds an instruction naming it in a confining role narrows it (`constrainRegClass`): `LirBody.classes: Arc<IndexMap<u32, ClassSet>>`.
- What stays an allocation-time analysis, on top of the stored classes: selector inference, the `[base+index]` two-colouring of pairs and the preference by clobbers (`regclass` steps 1-4). They read global structure and are not per-instruction.

## Who makes values (the places that must set or narrow a class)

| Creator | What it makes | Owner |
|---|---|---|
| isel.rs, isel/combined.rs, selectiondag/emit.rs | the values of the selected body | sdag (w9/w10/w12) |
| combiner.rs, division.rs, flagscopy.rs, timing.rs | helper values (shift counts, flag bytes) | w2/w10 |
| phielimination.rs, phiconst.rs | phi temporaries (copy of the result's class) | w1 |
| twoaddr.rs | tied copies, `lea` results (class of the source) | w1 |
| constrain.rs | `distinct_classes` copies (the narrowing is the point) | splitkit-port |
| coalescer (joins) | a join intersects the two classes | splitkit-port |
| greedy/inlinespiller.rs, liverangeedit.rs, spliteditor | spill and split products inherit the parent's class | splitkit-port |
| floatassign.rs, fpregs.rs, fpstack.rs | x87 values (no GPR class) | w2 |

## Steps

1. A helper `body.classes_of(value)` that scans lazily once per body generation (the cache `with_blocks` drops): no creator changes; measure how many rescans remain (the five callers see different bodies, so little is saved: this is why it is not enough).
2. Creators set the base class; a debug check compares the stored class with the scan (`LLRM_CHECK_CLASSES`) over the corpus.
3. Narrowing at the instruction builders and at joins; delete the scans RegAlloc, the coalescer and pressuresink make.

## Cost and risk

#3625 measured +0.40% Ir for computing a result's class at phielim: the stored class has to be cheap to keep (no per-value set allocation: a small enum of the target's classes). m16's classes (address-size prefix, `[bx+si]` forms) are the hard part. Gain: the 46 scans, ~10 M of d_faces -O2 (1.1%).
