// flags: -O0 | -O2 -march=i486 | -O0 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* A 64-bit return in EAX:EDX where each half came in the other register: under --isel=dag the ret required nothing and the first copy clobbered the second half. */
extern void report(long v);
unsigned long long pair(unsigned long a, unsigned long b) { return ((unsigned long long)a << 32) | b; }
int main(void) {
    unsigned long long r = pair(0x11111111UL, 0x22222222UL);
    report((long)(r >> 32));
    report((long)r);
    return 0;
}
