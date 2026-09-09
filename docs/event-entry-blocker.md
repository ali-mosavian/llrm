# Event-entry emission blocker

Current full `tools/opportunity.py --targets` exits nonzero. In particular,
PDS and VBDOS event-enabled ADDRM both refuse at the module's near call:
`0x0048: this call's interface is not established`. The local event stub's
far jump names B$EVK1, not B$EVCK. Many event-enabled rows share this blocker;
they must not be reported as completed or compared to event-free targets.

PUBDEF inspection establishes that the two names are exact aliases within
each shipped runtime: both name segment 1 offset 0103 in PDS and 0127 in
VBDOS, in the same `..\rt\evtcore.asm` module. The library SHA-256 values are:

- PDS BCL71ENR.LIB: `873fde67aa6fcf27961ec76d9f57ea8a621f6d16ea064da3312aa8d9e3a8c117`
- VBDOS VBDCL10E.LIB: `59ad49b055c4829528301e512abf9b8b0955181024c18282a49839e6c0680301`

The family-specific B$EVK1 contract therefore inherits the existing B$EVCK
interface, including arbitrary memory effects and entry into user code.
Unknown families and QB45 do not acquire this alias. This does not recognize
the compiler's near-call stub or prove equality between runtime versions;
the production refusal and before/after assembly remain unchanged.

The shipped PDS `BCL71ENR.LIB`, module `..\rt\evtcore.asm`, defines B$EVK1
at segment 1 offset 0103. Direct disassembly and the dependency-aware contract
tool establish two different paths:

```asm
0103 cmp word [pending],0
0108 jne 010b
010a retf
; pending path, selected instructions:
011e call ax             ; dispatch table entry
016d call far [handler]
0177 call far B$FRAMESETUP
017c inc word [bp-12h]
0186 jmp far [bx+1]      ; enters user code
```

Symbolic memory names above describe their role, not resolved relocation
names. The dependency report resolves the direct FRAMESETUP call; the indirect
calls and final transfer remain unknown. Analysis was bounded to 32 functions,
so recursive/budget-limited dependencies are also unresolved. It proves no
preserved register set or stack cleanup. The no-event RETF is insufficient
to assign a whole-function ABI. VBDOS defines a matching fast-path shape at
0127; that does not establish equality of the complete implementations.

Reproduce the PDS evidence:

```sh
uv run python tools/contracts.py 'B$EVK1' \
  --lib /Users/alim/work/other/d32x/toolchains/pds71/LIB/BCL71ENR.LIB \
  --functions 32 --dump /tmp/qbopt-evk1-pds.json
```

Required implementation work is to model the stub and event continuation,
including handler-visible memory, stack/frame transitions and register state
on resumption, against each runtime family. Only then can lowering consume
a justified interface and event-preserving performance targets be derived.
Do not broaden an ordinary call contract to conceal this control-flow edge.
This inspection changes no project output or score.
