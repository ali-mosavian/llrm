# QCport against main, 2026-10-21

All 65 modules, OMF object-file bytes of `llrm-cc` (tools/rows/qcport.py's compile: no flags, the sources of `src`; LEDATA bytes beside them). main = `854bb7efc4` (llrm-cc md5 56f120bb59fa); parity = `c4fade7345` (fresh dist build, llrm-cc md5 68c5bd191371). main builds all 65 (its total, 321,280, is oversight's in `qcport-os-3446.tsv`). An earlier note of mine that main fails 11 modules was my own `-m16 -march=i386` flags, not main.

## -Os

Total main 321280 B, parity 328111 B (+6831 B); 43 of 65 modules above main, 1 equal, 21 below.

Modules above main, largest first. Owner: the row that holds it; `no owner` is for the coordinator to assign.

| module | main B | parity B | delta | LEDATA delta | owner |
|---|--:|--:|--:|--:|---|
| host/host | 33681 | 34926 | +1245 | +1270 | no owner |
| game/mdl_ai | 25850 | 26461 | +611 | +703 | no owner |
| render/sc | 11678 | 12280 | +602 | +575 | no owner |
| render/d_alias | 13982 | 14546 | +564 | +507 | ALLOC-GAP / X87-GAP |
| game/triggers | 6604 | 7118 | +514 | +484 | no owner |
| host/demo | 7005 | 7423 | +418 | +394 | ALLOC-GAP / X87-GAP |
| ui/screen | 8899 | 9291 | +392 | +349 | no owner |
| render/r_bsp | 5553 | 5914 | +361 | +357 | no owner |
| ui/sbar | 4656 | 4970 | +314 | +179 | no owner |
| model/mdl | 8204 | 8505 | +301 | +289 | no owner |
| render/dl | 4625 | 4872 | +247 | +240 | no owner |
| ui/console | 4557 | 4802 | +245 | +246 | no owner |
| game/ent | 10251 | 10471 | +220 | +221 | no owner |
| game/plats | 2711 | 2931 | +220 | +220 | no owner |
| host/cmd | 4080 | 4271 | +191 | +195 | no owner |
| render/viewfx | 1757 | 1929 | +172 | +172 | no owner |
| game/pl_trace | 6140 | 6291 | +151 | +114 | no owner |
| game/doors | 3293 | 3416 | +123 | +90 | no owner |
| game/client | 2274 | 2394 | +120 | +119 | no owner |
| render/r_portal | 27303 | 27419 | +116 | +121 | no owner |
| model/mod_tex | 2299 | 2412 | +113 | +113 | no owner |
| model/mod | 3252 | 3358 | +106 | +106 | no owner |
| model/sprite | 1136 | 1237 | +101 | +101 | no owner |
| game/item | 6140 | 6237 | +97 | +76 | no owner |
| game/mover | 1681 | 1773 | +92 | +92 | no owner |
| host/pak | 2337 | 2407 | +70 | +70 | no owner |
| model/viewmdl | 611 | 681 | +70 | +65 | no owner |
| sound/snd | 4913 | 4981 | +68 | +17 | no owner |
| host/config | 2029 | 2094 | +65 | +57 | no owner |
| game/entitems | 1281 | 1345 | +64 | +64 | no owner |
| render/ls | 2300 | 2363 | +63 | +70 | no owner |
| render/d_faces | 6208 | 6262 | +54 | -32 | LU1 waiver |
| render/loadscr | 2171 | 2217 | +46 | +46 | no owner |
| ui/center | 407 | 440 | +33 | +33 | no owner |
| host/dbg | 638 | 670 | +32 | +32 | no owner |
| render/view | 1113 | 1144 | +31 | +31 | no owner |
| host/keybind | 4392 | 4421 | +29 | +47 | no owner |
| host/bench | 3068 | 3092 | +24 | +24 | no owner |
| render/d_sky | 821 | 845 | +24 | +24 | no owner |
| ui/menu | 6668 | 6687 | +19 | -7 | no owner |
| host/sys | 2413 | 2430 | +17 | +28 | no owner |
| render/vid | 5119 | 5136 | +17 | +17 | no owner |
| host/qmain | 442 | 457 | +15 | +11 | no owner |

Modules below main: host/savegame -510, render/part -329, game/weapons -119, render/r_walk -95, render/vshift -90, render/d_beam -65, game/combat -55, game/gib -49, host/sys_time -45, game/pl_move -41, render/sb_build -36, render/pal -24, render/d_poly -21, sound/snd_mix -16, render/d_sprite -13, game/lights -12, host/mathlib -10, game/enttext -6, render/qglsurf -5, host/rng -3, host/input -2

## -O2 (trend only: bytes do not bind there)

Total main 335803 B, parity 390976 B (+55173 B); 55 of 65 above main, 2 equal, 8 below.

Ten largest above main: game/mdl_ai +5858, ui/sbar +5652, host/cmd +5032, render/sc +3540, render/d_alias +2914, ui/screen +2618, game/weapons +2420, host/host +2398, ui/menu +2263, host/keybind +2159.

Files: `qcport-os-parity-vs-main.tsv`, `qcport-o2-parity-vs-main.tsv` (columns as oversight's: module, main, parity, delta; then the LEDATA columns).
