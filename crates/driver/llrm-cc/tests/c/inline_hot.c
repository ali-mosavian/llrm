/* m32 -O2 (INL-HOT): `mix` is over the pin's threshold at a site in a loop (cost 630 against 337)
   but pays: the trial saves its call, argument pushes and preserved-register
   pushes 31 times. `mixbig` costs 12 KB inlined and stays a call. */
extern void report(long);
#define R(k) a += (b ^ (c << ((k) % 7 + 1))) + (k) * 2654435u; b = (b << 5 | b >> 27) ^ a; c += d * ((k) * 2 + 3); d ^= c >> ((k) % 5 + 1);
#define R4(k) R(k) R(k + 1) R(k + 2) R(k + 3)
#define R16(k) R4(k) R4(k + 4) R4(k + 8) R4(k + 12)
#define R64(k) R16(k) R16(k + 16) R16(k + 32) R16(k + 48)
unsigned mix(unsigned a, unsigned b, unsigned c, unsigned d) { R16(0) return a ^ b ^ c ^ d; }
unsigned mixbig(unsigned a, unsigned b, unsigned c, unsigned d) { R64(0) R64(64) R64(128) R64(192) return a ^ b ^ c ^ d; }
unsigned drive(unsigned *v, long n) { unsigned acc = 0; long i; for (i = 0; i < n; ++i) acc += mix(v[i], acc, i, 7u); return acc; }
unsigned again(unsigned *v, long n) { unsigned acc = 0; long i; for (i = 0; i < n; ++i) acc ^= mix(acc, v[i], 3u, i); return acc; }
unsigned driveb(unsigned *v, long n) { unsigned acc = 0; long i; for (i = 0; i < n; ++i) acc += mixbig(v[i], acc, i, 7u); return acc; }
unsigned againb(unsigned *v, long n) { unsigned acc = 0; long i; for (i = 0; i < n; ++i) acc ^= mixbig(acc, v[i], 3u, i); return acc; }
int main(void) { unsigned v[8] = {1, 2, 3, 4, 5, 6, 7, 8}; report(drive(v, 8) + again(v, 8) + driveb(v, 8) + againb(v, 8)); return 0; }
