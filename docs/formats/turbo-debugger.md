# Turbo Debugger information (`-gtd`)

Turbo C++ 3.0 writes its debug information into the object as `COMENT` records (attribute 0, so
"Purge: Yes, List: Yes") of classes 0xE1-0xEE; `TLINK /v` turns them into the table Turbo Debugger reads.
Measured on `tcc -v` objects (TC++ 3.0, `-ml`..`-mh`, `-a`) with `TDUMP` as the reader and by linking the
object and dumping the EXE (`tdump x.exe` prints the table, so what TLINK built is what the debugger gets).
llrm writes the same records for a 16-bit C object: `llrm-omf/src/td.rs`. The two agree on the Module
Table of `tests/turbo.rs`'s programs, the oracle.

## Records

| class | meaning | data |
| --- | --- | --- |
| E1 | a public symbol's type, after its PUBDEF | `index, flag` (0 data, 0x18 function) |
| E2 | a struct's or union's members, before its E3 | `(bits, name, type [, 0x40 offset32])*, 0xC0 size32` |
| E3 | a type | `index, name, size16, tid, ...` |
| E5 | begin scope | `segment, offset16` |
| E6 | locals, statics, type names | `(name, type, flag, ...)*` |
| E7 | end scope | `offset16` |
| E8 | source file, before LINNUM | `0, name, time32` |
| EA | compiler parameters | `language (1 C), model (8 tiny .. 0xD huge)` |

- An index is one byte below 0x80, else `0x80 | high, low` (150 structs reach `80 AD`, 400 `81 A7`). The
  scalars' are fixed: void 1, char 2, int 4, long 6, unsigned char 8, unsigned 0xA, unsigned long 0xC,
  float 0xE, double 0xF, long double 0x10; a type's own begin at 0x18, in order of first use. A pointer to a
  struct is named before the struct's record, so a cycle closes.
- E3 `tid`: 0x15 near pointer, 0x16 far (1 huge), 0x1A array, 0x1E struct, 0x1F union, 0x22 enum, 0x23 function.
  Pointer: `size, tid, target, kind` (kind 4 near data, 2 near code; far: 0 or 1 for huge). Array: `bytes,
  0x1A, element`. Function: `0, 0x23, return, call, 0` (call: 4 far, 0 near, +1 Pascal). Qualifiers leave no
  trace; an enum's record has no enumerators.
- Struct members carry no offsets but where one is not where the last ended (`0x40, offset`). A bit field
  is `width` in the first byte.
- A function is a scope at its first byte holding its parameters last first (flag 0x0A, BP offset), then a
  scope at its body's first byte holding the locals (flag 0x02, BP offset) and the parameters again, both
  ended at its last byte (E7 twice). A block is a scope inside. A register variable is flag 4 or 0xC and the
  register's number (SI 6, DI 7, BL 3). A static is `name, type, 0, 1, segment, offset16`; a static function
  `name, type, 0x18, 0, segment, offset16`. Struct tags are flag 7 and typedef names flag 6 in a module-level
  E6, after the PUBDEFs.
- One PUBDEF a symbol, its E3 and E1 right after. Turbo C++ also writes dependency (E9) and coverage (EE)
  records, which llrm does not.

## Open

Register variables (a register's number is the target's, not a column of `registers.regs` yet), bit fields
beside padding (the encoding of `int a:3; char c;` is not known), enums, coverage offsets, 32-bit objects
(Borland's are another format), BASIC and Nib programs, a type past 64K (`huge`'s 80,400-byte array).
Turbo Debugger 3.00 itself is driven by `tests/turbo.rs` under the fork's DOSBox-X debug socket
(`DOSBOX_DEBUG_PORT`: key injection, the text screen, a PNG screenshot): it stops at every INT 3, which
Turbo Debugger uses for its breakpoints, so a reader thread answers each with `continue`. F8 and F7 step
to a line (the window's title says which), Ctrl-F7 (scan code 0x64, not 0x6A) adds a watch. For the
program built by Turbo C++ and the one llrm built with `-gtd`, stopped at `lst.c` line 9, the Watches window
reads alike: `s int 0`, `n struct node * ds:00AE [_head]`, `*n struct node {ds:00AA,3}`, `n->v int 3`.
