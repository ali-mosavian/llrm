/* The clock tick of a host (18.2 a second), which calls music_tick (play.c) as the timer interrupt does on DOS.
   dev_tick_hold keeps it out of a critical section by taking the lock the tick takes, so the hold of one thread
   waits for no one but a tick under way. Not nested. */
#ifndef QB_HOST_TICK_H
#define QB_HOST_TICK_H

/* Whether dev_tick_start starts a thread that ticks in real time (a window's), or leaves ticks to host_tick_fire
   (a test's). */
void host_tick_set_thread(int yes);

/* One tick now, if the tick is started and not held; whether it ran. */
int host_tick_fire(void);

#endif
