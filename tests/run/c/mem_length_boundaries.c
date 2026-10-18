// flags: -O2 -march=i486
// targets: x86-m16
// link: mem_length_boundaries_src.c
/* A constant fill or copy length is unsigned: 32768 and 65535 bytes read as negative 16-bit numbers stored nothing, so the
   loops below (made into memset and memcpy) left their arrays as they were. Two far arrays of 65535 bytes, each in a segment
   of its own (`src` is in the file linked beside). */
extern void report(long value);
extern unsigned char __far src[65535];
unsigned char __far dst[65535];

#define FILL(name, n) static void name(void) { unsigned i; for (i = 0; i < n##u; i++) dst[i] = 2; }
#define COPY(name, n) static void name(void) { unsigned i; for (i = 0; i < n##u; i++) dst[i] = src[i]; }
FILL(fill_32767, 32767)
FILL(fill_32768, 32768)
FILL(fill_65535, 65535)
COPY(copy_32767, 32767)
COPY(copy_32768, 32768)
COPY(copy_65535, 65535)

static unsigned long total(void)
{
    unsigned long sum = 0;
    unsigned i;
    for (i = 0; i < 65535u; i++) sum += dst[i];
    return sum;
}

int main(void)
{
    unsigned i;
    for (i = 0; i < 65535u; i++) src[i] = (unsigned char)((i & 3) + ((i >> 2) & 1));
    fill_32767(); report((long)total());
    fill_32768(); report((long)total());
    fill_65535(); report((long)total());
    for (i = 0; i < 65535u; i++) dst[i] = 0;
    copy_32767(); report((long)total());
    copy_32768(); report((long)total());
    copy_65535(); report((long)total());
    return 0;
}
