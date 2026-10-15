# d_faces, executed

The code-quality scoreboard for one file: QCport's `render/d_faces.c`, the per-frame face loop the user's goal is judged on. vsgcc's other programs measure kernels written for the comparison; this runs the real file, on a scene that reaches each of its paths, and counts what executes, not what is emitted, so that a loop weighs as often as it runs.

    uv run --project tools python crates/target/llrm-x86-m32/vsgcc/dfaces/run.py [--levels O1 O2 O3 Os] [--json OUT]
    uv run --project tools --group dev pytest crates/target/llrm-x86-m32/vsgcc/dfaces

About a minute. Products go to `$VSGCC_WORK` (default `~/scratch/vsgcc-work`); the QCport sources are read from `$QCPORT` (default `~/scratch/qbq-llrm/src`).

| File | Does |
|---|---|
| `run.py` | builds every variant, links it with `env.c`, runs it in unicorn, prints the table |
| `env.c` | the world d_faces.c runs in: the scene, and stand-ins for what the file calls |
| `test_dfaces.py` | the scene reaches every path; an optimised build reports the reference's hash; a changed file is caught |

## How it measures

- **Variants.** llrm (`-m32 -mabi=sysv`), gcc and clang at -O1, -O2, -O3 and -Os, all with `-march=i486`; gcc -O0 is the reference. Every compiler reads the same wrapper (`d_faces.c` with `far`, `near`, `huge`, `pascal` and `cdecl` emptied), so that all take the flat model and one calling convention.
- **What is counted.** Every instruction executed inside d_faces.o and the runtime library it pulls in; the scene and the stand-ins (env.elf, loaded apart) are not. Instructions, memory operands, operands across an alignment boundary, and 486 clocks by [harness.py](../harness.py)'s model.
- **Misaligned operands.** Intel486 data sheet 240440-002 (10.1, assumption 2, and Table 2.1): a word is aligned at 2, a dword or single real at 4, a double or extended real at 8, and each misaligned access costs three clocks, which `harness.py` adds to the clocks and counts apart.
- **Self-check.** `env.c` folds every call it receives (integers exactly, floats rounded coarsely enough that the x87's extended intermediates and a float rounded at every store agree) into one hash, and the stand-ins answer from their arguments alone. A variant is `yes` when it reports the hash of gcc -O0, `WRONG` when not; the exit status says so.

## The scene

`bench_dfaces` runs five frames over 96 seeded faces in 16 nodes: perspective, affine and wireframe modes; with and without depth (front to back, then back to front); no destination; two cameras; the profile clock on once. The faces are triangles to hexagons, a 2-corner record (clipped away), 33-corner records, one record that meets the end of its row, faces across the near and far planes and behind the camera, both sides of a plane, brush entities, lit and unlit faces (the surface cache hits, misses, allocates and fails to), liquid and animated textures. `coverage` in `env.c` counts the paths the stand-ins saw, the run prints them, and `test_dfaces.py` fails if one is zero.

## Caveats

- The scene is synthetic. It is seeded and fixed, so the ratios are comparable between compilers and between runs, but they weigh the paths as the scene exercises them, not as a game frame does. A captured QCport frame (the order list, the face and texture tables, the camera) can replace `build()` in `env.c`.
- The stand-ins are cheap and identical for every variant; calls into them are charged as taken calls, nothing more.
- m16 is not run here yet: its far pointers need the scene in a segment of its own.
