# Timed benchmark objects

Real VBDOS 1.00 Professional output, not synthesized or patched, kept as OMF inputs for `llrm-omf`.

- `nbody-v-g3.obj`: `bench/nbody.bas`, compiled with `/O /FPi /R /G3 /E /Zi`, zero warnings and severe errors. It retains the PIT reader as well as the simulation and references runtime data `B$SEG` through an external OFFSET16 relocation.
- `fpbench-v-g3.obj`: `bench/fpbench.bas`, same compiler and switches.
- `nbodys-v-g3.obj`: `bench/nbodys.bas`, the integrator with its LONG state as SINGLE, same compiler and switches.
