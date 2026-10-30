/* The device interface's functions that the target's assembly provides, by their assembly names. */
#pragma aux dev_tick_start "B$TICKON"
#pragma aux dev_tick_stop "B$TICKOFF"
#pragma aux dev_tick_hold "B$CLI"
#pragma aux dev_tick_release "B$STI"
