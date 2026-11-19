# A node simplified by demanded bits is revisited (2026-11-12)

Tip 654329cfe9 build (dist-base) to the same plus the change (dist-mine), same harness, 4-pad mean. No 6502 cell is worse. Programs outside the 79-cell table that the change touches (measured the same way):

| program | level | bytes | clocks |
|---|---|---|---|
| bitcount_u64 | O1, O2 | 4557 -> 4556 | 58922 -> 58794 |
| bitcount_u64 | Os | 4672 -> 4671 | 62152 -> 62027 |
| clzll_ctzll_64 | O1, O2 | 2222 -> 2197 | 9327 -> 9242 |
| clzll_ctzll_64 | Os | 2270 -> 2245 | 9758 -> 9674 |

Other targets (-S over tests/run/c at -O2, byte-equal to the tip except): x86_32-linux high_byte (`shr ecx, 8; movsx eax, cl` -> `mov eax, ecx; sar eax, 8`, -1 B) and arm64-linux wc_mixed (`madd w9, w9, w11, wzr; add` -> one `madd`, one instruction fewer). m68k-68000, wasm32, x86_64-linux and z80-sdcc: 0 programs differ. x86_16-dos was not compared (every program failed to compile in the harness on both sides).

```
== O1: 1 cells changed, dB -215; bytes-worse 0, clk-worse 0
  wide_shift_by_one                    1204->   989 B      405957->    171729 clk  -57.7% 
== O2: 1 cells changed, dB -215; bytes-worse 0, clk-worse 0
  wide_shift_by_one                    1204->   989 B      405957->    171729 clk  -57.7% 
== Os: 2 cells changed, dB -218; bytes-worse 0, clk-worse 0
  frameless                            2392->  2390 B       82042->     80854 clk   -1.4% 
  wide_shift_by_one                    1231->  1015 B      612797->    182377 clk  -70.2% 
```
