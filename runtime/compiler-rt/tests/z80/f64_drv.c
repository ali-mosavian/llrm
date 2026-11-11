typedef unsigned long long u64;
u64 __muldf3(u64, u64);
u64 __divdf3(u64, u64);
#ifndef OP
#define OP __muldf3
#endif
#ifndef NV
#define NV 32
#endif
volatile u64 sink;
u64 vec[NV][2];
int main(void) {
    u64 s = 0x9E3779B97F4A7C15ull, acc = 0; int i;
    for (i = 0; i < NV; i++) {
        s ^= s << 13; s ^= s >> 7; s ^= s << 17; vec[i][0] = (s & 0x800FFFFFFFFFFFFFull) | ((u64)(1010 + ((s >> 52) & 0x1F)) << 52);
        s ^= s << 13; s ^= s >> 7; s ^= s << 17; vec[i][1] = (s & 0x800FFFFFFFFFFFFFull) | ((u64)(1010 + ((s >> 52) & 0x1F)) << 52);
    }
    for (i = 0; i < NV; i++) acc ^= OP(vec[i][0], vec[i][1]);
    sink = acc;
    *(volatile u64 *)0x8f00 = acc;
    return 0;
}
