# Machine

What the x86 backend must know about the targets and their hardware. For contributors to isel, allocation and the targets.

| Doc | Holds |
|---|---|
| [targets](targets.md) | The targets built, their crates and description files, and how to add one |
| [a32](a32.md) | 386 addressing in real mode behind the `67h` prefix |
| [metal](metal.md) | The operand-size prefix and what only hardware can answer |
| [pointer-lowering](pointer-lowering.md) | Huge pointers: carry into the selector, windows |
| [arithmetic-targets](arithmetic-targets.md) | CPU-dependent constant multiply and divide |

The calling conventions of each target are in [abi](../abi/readme.md).
