# bcobj

The BC object frontend: it reads an OMF object that QuickBASIC's BC wrote, decodes its x86 code, and lifts each body to regular llrm MIR. It emits nothing target-specific: no opcode, type, attribute or intrinsic of its own, and no `llvm.x86.*` or inline assembly. `tests/pure_ir.rs` checks this on every sample object.

## Address spaces

Real mode's pointers are the one target-flavoured element in the emitted IR. The numbers come from the target's description (`datalayout.toml`, `[spaces]`), never from this crate; for real mode they are:

| Space | Role |
|---|---|
| 0 | near: an offset in DS |
| 1 | far: offset and segment |
| 2 | a segment alone (DGROUP's selector); a cast from a far pointer gives its segment, one back gives segment:0 |
| 3 | huge: a far pointer whose offset carries into its segment |
| 5 | the stack |

Casts between them are `inttoptr` and `addrspacecast`.
