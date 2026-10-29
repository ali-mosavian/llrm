# Docs

For readers and contributors of llrm. Read in this order; each section's readme lists its documents.

```
 readme.md -> architecture -> frontends -> optimizations -> formats
  (what, how    (pipeline,    (QB, Nib, C,   (one note per     (objects,
   to build)     MIR, HIR)     BC objects)    optimization)     debug info)
                                                    |
                              working <- reference <- machine
                           (test, debug,  (levels,    (x86 real mode,
                            measure)       switches,   targets; abi)
                                           tools, crates)
```

| Section | Holds |
|---|---|
| [root readme](../readme.md) | What llrm is, its programs, how to build and use them |
| [architecture](architecture/readme.md) | The pipeline, the MIR boundary, HIR, analyses, source layout, packages |
| [frontends](frontends/readme.md) | [QuickBASIC](frontends/qb/readme.md), [Nib](frontends/nib/readme.md), [C](frontends/c/readme.md), and [BC objects](frontends/bc.md) |
| [optimizations](optimizations/readme.md) | One note per optimization, each measured on a program |
| [formats](formats/readme.md) | The object model and its writers; debug information |
| [machine](machine/readme.md) | x86 real mode: prefixes, pointer lowering, targets |
| [register allocation](machine/register-allocation/readme.md) | How values get registers and frame slots, and how to debug it |
| [abi](abi/readme.md) | Calling conventions: registers, stacks, near and far calls, how a convention is chosen, the tests that pin them |
| [reference](reference/readme.md) | Optimization levels, the switches that change what a program means, the tools, the crates |
| [comparison](comparison/readme.md) | llrm's output beside other compilers' |
| [working](working/readme.md) | Testing, debugging, measuring, and what has broken before |

Contributors also read [agents.md](../agents.md): the rules and method.
