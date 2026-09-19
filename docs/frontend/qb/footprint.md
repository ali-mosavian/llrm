# QB frontend code-footprint ledger

Code footprint is measured from Microsoft LINK's MAP file, not from `.OBJ`
file sizes. An object contains data, symbols, relocations, and OMF framing;
its byte length is not the amount of executable memory the linked program
occupies. Every current measurement reports two scopes from the exact linked
hexadecimal lengths in the MAP segment table:

- `BC_CODE` is the per-module, BASIC-owned code. It identifies which frontend
  output grew or shrank.
- `BC_CODE + CODE` is the complete linked executable code. It includes runtime
  helpers pulled in by BASIC calls as well as the unchanged C and assembly
  inputs. This is the primary final-size comparison: a five-byte call is not
  allowed to hide the helper behind it.

The tool rejects MAP files containing linker errors. The all-BC qrender
control is `build/qblegacy-link/QRENDER.MAP`; it linked and ran with the same
response file, UGL library, and non-BASIC objects as the all-qbopt image.

Generate the same comparison with:

```text
uv run tools/qbfootprint.py path/to/bc/QRENDER.MAP path/to/qbopt/QRENDER.MAP
```

The tool reports each BASIC module, the BASIC-owned total, and the complete
linked-code total. The first three historical tables below predate the second
scope and retain only their preserved `BC_CODE` readings; no complete-code
number is inferred for a MAP that was not saved.

## 2026-09-19: first all-qbopt qrender link

Both links use the VBDOS runtime and the same qrender source and unchanged
C/assembly objects. The qbopt build completes initialization through the
`colormap` marker but has not yet completed its first rendered frame. A mixed
link with the eight frame-path modules replaced by BC output completes five
frames. Therefore the table is a size baseline and a regression gate, not a
claim that the all-qbopt image is functionally complete.

| Module | BC bytes | qbopt bytes | Delta |
| --- | ---: | ---: | ---: |
| COMMON | 2,753 | 3,018 | +265 |
| D_POLY | 2,081 | 6,987 | +4,906 |
| D_SURF | 12,570 | 29,310 | +16,740 |
| ENT | 4,808 | 10,787 | +5,979 |
| H_BENCH | 6,585 | 7,825 | +1,240 |
| H_FRAME | 2,567 | 2,435 | -132 |
| IN_MAIN | 599 | 642 | +43 |
| MAIN | 5,208 | 6,410 | +1,202 |
| MODEL | 4,121 | 4,115 | -6 |
| MOD_TEX | 3,086 | 5,248 | +2,162 |
| PL_MOVE | 5,142 | 4,902 | -240 |
| R_BSP | 5,166 | 9,389 | +4,223 |
| SCREEN | 13,591 | 20,011 | +6,420 |
| SND | 1,097 | 1,303 | +206 |
| SYS | 3,419 | 4,469 | +1,050 |
| VID | 664 | 590 | -74 |
| VIEW | 3,159 | 4,647 | +1,488 |
| **BASIC-owned BC_CODE** | **76,616** | **122,088** | **+45,472** |

That all-qbopt image was **59.4% larger** in linked BASIC-owned code. This is
a measured code-quality regression. Each later integration round must keep a
copy of its MAP comparison beside the functional output; a correctness fix
must not silently erase or disguise a footprint change.

## 2026-09-19: split huge pointers for numeric array formals

Raw VBDOS `R_BSP` uses selector `AD+2` and adjusted offset `AD+0Ah` for an
incoming numeric array descriptor. Correcting the frontend's former packed
load from `AD+0` increases the linked result because pointer reconstruction
and normalization are repeated at each access. The full corrected module set
links successfully; following the listing-first diagnosis, it has not yet had
another long DOSBox run.

| Module | BC bytes | qbopt bytes | Delta |
| --- | ---: | ---: | ---: |
| COMMON | 2,753 | 3,018 | +265 |
| D_POLY | 2,081 | 8,622 | +6,541 |
| D_SURF | 12,570 | 28,449 | +15,879 |
| ENT | 4,808 | 14,552 | +9,744 |
| H_BENCH | 6,585 | 7,852 | +1,267 |
| H_FRAME | 2,567 | 2,435 | -132 |
| IN_MAIN | 599 | 642 | +43 |
| MAIN | 5,208 | 6,513 | +1,305 |
| MODEL | 4,121 | 4,237 | +116 |
| MOD_TEX | 3,086 | 5,386 | +2,300 |
| PL_MOVE | 5,142 | 4,902 | -240 |
| R_BSP | 5,166 | 11,029 | +5,863 |
| SCREEN | 13,591 | 20,416 | +6,825 |
| SND | 1,097 | 1,303 | +206 |
| SYS | 3,419 | 4,532 | +1,113 |
| VID | 664 | 590 | -74 |
| VIEW | 3,159 | 4,815 | +1,656 |
| **BASIC-owned BC_CODE** | **76,616** | **129,293** | **+52,677** |

This is **68.8% above BC** and 7,205 bytes above the preceding qbopt round.
The next size work is general CSE/placement of loop-invariant descriptor base
construction in existing MIR. The semantic fix must not be replaced by the
smaller but incorrect `AD+0` load.

## 2026-09-19: reuse descriptor data bases within effect-free regions

The QB semantic builder now resolves one descriptor data base per straight-line
region and reuses the typed HIR value for subsequent element accesses. Any
user/runtime call or CFG boundary invalidates the value, so a callee that
REDIMs an aliased array cannot leave a stale pointer behind. The isolated
fixture has three accesses, a call, and three more accesses: it emits exactly
two `AD+2`/`AD+0Ah` reconstructions.

| Module | BC bytes | qbopt bytes | Delta |
| --- | ---: | ---: | ---: |
| COMMON | 2,753 | 3,026 | +273 |
| D_POLY | 2,081 | 7,165 | +5,084 |
| D_SURF | 12,570 | 28,390 | +15,820 |
| ENT | 4,808 | 12,558 | +7,750 |
| H_BENCH | 6,585 | 7,844 | +1,259 |
| H_FRAME | 2,567 | 2,435 | -132 |
| IN_MAIN | 599 | 642 | +43 |
| MAIN | 5,208 | 6,495 | +1,287 |
| MODEL | 4,121 | 4,139 | +18 |
| MOD_TEX | 3,086 | 5,312 | +2,226 |
| PL_MOVE | 5,142 | 4,902 | -240 |
| R_BSP | 5,166 | 9,924 | +4,758 |
| SCREEN | 13,591 | 20,580 | +6,989 |
| SND | 1,097 | 1,303 | +206 |
| SYS | 3,419 | 4,549 | +1,130 |
| VID | 664 | 590 | -74 |
| VIEW | 3,159 | 4,887 | +1,728 |
| **BASIC-owned BC_CODE** | **76,616** | **124,741** | **+48,125** |

This recovers **4,552 linked bytes** and leaves qbopt **62.8% above BC**.
The small D_SURF change is consistent with its dense call boundaries; the
large D_POLY, ENT, and R_BSP reductions confirm that repeated descriptor-base
construction was real code, not an object-format artifact.

## 2026-09-19: reuse descriptor bounds within the same regions

The same invalidation mechanism now covers descriptor dimension fields.
Repeated accesses no longer reload the same lower bound or element count until
a call or CFG boundary can invalidate the descriptor. This is one descriptor
snapshot mechanism, shared by data-base and bound loads.

| Module | BC bytes | qbopt bytes | Delta |
| --- | ---: | ---: | ---: |
| COMMON | 2,753 | 3,024 | +271 |
| D_POLY | 2,081 | 5,050 | +2,969 |
| D_SURF | 12,570 | 28,390 | +15,820 |
| ENT | 4,808 | 11,958 | +7,150 |
| H_BENCH | 6,585 | 7,844 | +1,259 |
| H_FRAME | 2,567 | 2,435 | -132 |
| IN_MAIN | 599 | 642 | +43 |
| MAIN | 5,208 | 6,495 | +1,287 |
| MODEL | 4,121 | 4,139 | +18 |
| MOD_TEX | 3,086 | 5,009 | +1,923 |
| PL_MOVE | 5,142 | 4,902 | -240 |
| R_BSP | 5,166 | 8,384 | +3,218 |
| SCREEN | 13,591 | 20,580 | +6,989 |
| SND | 1,097 | 1,303 | +206 |
| SYS | 3,419 | 4,549 | +1,130 |
| VID | 664 | 590 | -74 |
| VIEW | 3,159 | 4,887 | +1,728 |
| **BASIC-owned BC_CODE** | **76,616** | **120,181** | **+43,565** |
| **Complete linked code** | **255,047** | **292,210** | **+37,163** |

This removes another **4,560 BASIC-owned bytes** and leaves that scope **56.9%
above BC**. D_POLY drops 2,115 bytes and R_BSP drops 1,540. D_SURF remains
exactly 28,390 bytes, so its remaining gap requires a different mechanism.

The primary complete-code result is **37,163 bytes, or 14.6%, above BC**.
The non-`BC_CODE` part is 178,431 bytes for BC and 172,029 for qbopt: qbopt
pulls in 6,402 fewer bytes of helper code because numeric operations are
inline. Reporting only the module total therefore overstated the final-size
regression, while also failing to price BC's helper calls at all.

## 2026-09-19: adjusted-base split-far indexing

The VBDOS descriptor's `AD+0Ah` field is already adjusted for declared lower
bounds. The frontend formerly subtracted `AD+10h` again, then represented the
ordinary default-model array address as a huge pointer. The corrected HIR adds
the scaled subscript to the 16-bit adjusted offset and concatenates the
unchanged selector. `/AH` remains a separate future policy.

| Module | BC bytes | qbopt bytes | Delta |
| --- | ---: | ---: | ---: |
| COMMON | 2,753 | 2,941 | +188 |
| D_POLY | 2,081 | 4,451 | +2,370 |
| D_SURF | 12,570 | 20,381 | +7,811 |
| ENT | 4,808 | 9,639 | +4,831 |
| H_BENCH | 6,585 | 7,670 | +1,085 |
| H_FRAME | 2,567 | 2,435 | -132 |
| IN_MAIN | 599 | 642 | +43 |
| MAIN | 5,208 | 5,793 | +585 |
| MODEL | 4,121 | 4,029 | -92 |
| MOD_TEX | 3,086 | 4,078 | +992 |
| PL_MOVE | 5,142 | 4,902 | -240 |
| R_BSP | 5,166 | 6,733 | +1,567 |
| SCREEN | 13,591 | 17,540 | +3,949 |
| SND | 1,097 | 1,303 | +206 |
| SYS | 3,419 | 4,899 | +1,480 |
| VID | 664 | 590 | -74 |
| VIEW | 3,159 | 3,507 | +348 |
| **BASIC-owned BC_CODE** | **76,616** | **101,533** | **+24,917** |
| **Complete linked code** | **255,047** | **273,562** | **+18,515** |

This removes **18,648 bytes** from both scopes. The BASIC-owned gap is now
32.5%; the primary complete-code gap is **7.3%**. D_SURF alone loses 8,009
bytes, confirming that generic huge-pointer normalization—not object framing—
was its dominant measured inflation. A live `dm3ish.bsp` run still reaches the
`colormap` marker and remains black before the first frame, so footprint and
runtime status remain separate gates.
