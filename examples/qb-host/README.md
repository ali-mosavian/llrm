# QB host demos

NIBBLES and GORILLAS for the native hosts (`llrm-qb --arm64=darwin|linux`, linked with `tools/link-qb-sdl.sh`).
They differ from the DOS originals only in their timing: CPU-speed delay loops are `TIMER` waits (`Rest`, NIBBLES'
game delay), the speed calibration is gone, and GORILLAS' explosions wait a tick per frame so they can be seen.
