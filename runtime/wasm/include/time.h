#ifndef LLRM_WASM_TIME_H
#define LLRM_WASM_TIME_H
typedef long time_t;
struct timespec {
    time_t tv_sec;
    long tv_nsec;
};
#define CLOCK_MONOTONIC 1
int clock_gettime(int clock, struct timespec *now);
int nanosleep(const struct timespec *wait, struct timespec *left);
#endif
