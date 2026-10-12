# Docs

For readers and contributors of llrm. Start at the [root readme](../readme.md) for what
llrm is and how to build it.

| Section | Holds |
|---|---|
| [architecture](architecture/readme.md) | The pipeline, the MIR boundary, HIR, analyses, source layout, packages |
| [frontends](frontends/nib/readme.md) | [QuickBASIC](frontends/qb/readme.md), [Nib](frontends/nib/readme.md), and [what the Open Watcom C front end gathers](frontends/c/ow-contract.md) |
| [optimizations](optimizations) | One note per optimization, each measured on a program |
| [machine](machine) | x86 real mode: ABIs, prefixes, pointer lowering, targets |
| [formats](formats/object-model.md) | The object model and its writers; debug information |
| [reference](reference/levels.md) | Optimization levels and the switches that change what a program means |
| [working](working/testing.md) | Testing, debugging, measuring, and what has broken before |
