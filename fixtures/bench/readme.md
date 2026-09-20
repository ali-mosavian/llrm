# Timed benchmark object

`nbody-v-g3.obj` is real VBDOS 1.00 Professional output from
`bench/nbody.bas`, compiled on 2026-09-09 with
`/O /FPi /R /G3 /E /Zi`. BC reported zero warnings and zero severe errors.
It retains the PIT reader as well as the simulation; unlike the small
operator corpus, it references runtime data `B$SEG` through an external
OFFSET16 relocation. No bytes were synthesized or patched.

`fpbench-v-g3.obj` is the corresponding real build of `bench/fpbench.bas`,
with the same compiler, switches and zero compiler errors, also built on
2026-09-09. It is retained for the current floating-point benchmark and
optimization investigation; `docs/numbers.md` records its hash and results.

`nbodys-v-g3.obj` is the same compiler's build of `bench/nbodys.bas`, the
integrator with its LONG state as SINGLE, built on 2026-09-13 with the same
switches and zero compiler errors, while the program still timed itself with
the PIT. It is the float MIR work's inner loop.
