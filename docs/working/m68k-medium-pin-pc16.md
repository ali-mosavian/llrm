# The pin's medium model at the 68020 cannot link past 32 KiB (2026-11-09)

TL;DR: the pin emits `lea (d16,%pc)` with an `R_68K_PC16` relocation for data at the 68020 in the medium model; two 70016-byte arrays fail to link.

Pin: LLVM 24.0.0git, `llc` of the m68k build (~/scratch/llvm-build-m68k), source d1106deb71cc.

```
clang --target=m68k-linux-gnu -mcpu=68020 -O0 -S -emit-llvm tests/run/c/memcpy_runtime.c -o pin.ll
llc -mtriple=m68k-linux-gnu -mcpu=M68020 -code-model=medium -O0 -filetype=obj pin.ll -o pin.o
m68k-linux-gnu-gcc -m68020 -nostdlib -static -ffreestanding pin.o -o pin.exe
```

```
pin.o: in function `main':
memcpy_runtime.c:(.text+0xa0): relocation truncated to fit: R_68K_PC16 against `.bss'
memcpy_runtime.c:(.text+0xdc): relocation truncated to fit: R_68K_PC16 against `.bss'
memcpy_runtime.c:(.text+0x114): relocation truncated to fit: R_68K_PC16 against `.bss'
memcpy_runtime.c:(.text+0x126): relocation truncated to fit: R_68K_PC16 against `.bss'
```

`llvm-objdump -dr pin.o`: `41 fa 00 00  lea (0,%pc), %a0` with `R_68K_PC16 .bss`. The pin has no `(bd32,pc)` form for
data (`MxPCD` is `(d16,pc)`, M68kInstrInfo.td L391/L406; the 68020 `MxPCIBD32` form is indexed, L417-422), though
`classifyLocalReference` picks PC-relative for medium at the 68020 (M68kSubtarget.cpp L199-208).
