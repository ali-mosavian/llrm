# Timed benchmark object

`nbody-v-g3.obj` is real VBDOS 1.00 Professional output from
`bench/nbody.bas`, compiled on 2026-09-09 with
`/O /FPi /R /G3 /E /Zi`. BC reported zero warnings and zero severe errors.
It retains the PIT reader as well as the simulation; unlike the small
operator corpus, it references runtime data `B$SEG` through an external
OFFSET16 relocation. No bytes were synthesized or patched.
