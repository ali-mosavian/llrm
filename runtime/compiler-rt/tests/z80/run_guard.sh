#!/bin/sh
# run.sh must not hang: 9000 f32 vectors (108 KB, past the 64 KB address space and the results at 0x8f00) once ran
# ucsim_z80 for 2.5 h, restarting main forever and reporting nothing. Now the size check fails first, and a run that
# does not finish in RUN_TIMEOUT s fails the run (SIM stands in a simulator that loops forever).
cd "$(dirname "$0")"; T=$(mktemp -d); trap 'rm -r $T' EXIT
o=$(NV=9000 timeout 120 ./run.sh fadd 2>&1); rc=$?
case $rc:$o in 1:*"past the results"*) ;; *) echo "oversize vectors: rc=$rc: $o"; exit 1;; esac
printf ':0200000018FEE8\n:00000001FF\n' > $T/loop.ihx
printf '#!/bin/sh\nexec ucsim_z80 -t Z80 -q -w %s/loop.ihx -c -\n' $T > $T/sim.sh; chmod +x $T/sim.sh
o=$(NV=50 SIM=$T/sim.sh RUN_TIMEOUT=3 timeout 120 ./run.sh fadd 2>&1); rc=$?
case $rc:$o in 1:*"did not finish"*) ;; *) echo "looping sim: rc=$rc: $o"; exit 1;; esac
o=$(NV=50 timeout 120 ./run.sh fadd 2>&1)
case $o in "fadd fails=0 "*) echo "run_guard ok";; *) echo "plain run: $o"; exit 1;; esac
