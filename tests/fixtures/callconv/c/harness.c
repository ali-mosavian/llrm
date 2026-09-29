/* Built by BCC alone: the inputs, and the check of what arrived and came
   back. Defines VARIADIC as its convention's files do. */
#include <stdio.h>
#include <string.h>
#include "cases.h"

signed char in_sc = -2;
unsigned char in_uc = 0xE7;
char in_ch = -3;
int in_i = -0x1235;
unsigned in_u = 0xBEEF;
long in_l = -0x12345679L;
unsigned long in_ul = 0xFEDCBA98UL;
char near *in_np = (char near *)0x2468;
char far *in_fp = (char far *)0x13572468UL;
char huge *in_hp = (char huge *)0x9ABC1357UL;
float in_f = 1.5f;
double in_d = -2.25;
long double in_ld = 3.125L;
S1 in_s1 = {{0x11}};
S2 in_s2 = {{0x21, 0x22}};
S3 in_s3 = {{0x31, 0x32, 0x33}};
S4 in_s4 = {{0x41, 0x42, 0x43, 0x44}};
S5 in_s5 = {{0x51, 0x52, 0x53, 0x54, 0x55}};
S7 in_s7 = {{0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77}};
S8 in_s8 = {{0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88}};
S9 in_s9 = {{0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99}};
SCI in_sci = {0x5C, 0x1C2C};

static int failures;

static void compare(const char *what, const void *got, const void *want, unsigned size) {
    unsigned at;
    if (memcmp(got, want, size) == 0) return;
    failures++;
    printf("FAIL %s got", what);
    for (at = 0; at < size; at++) printf(" %02X", ((unsigned char *)got)[at]);
    printf(" want");
    for (at = 0; at < size; at++) printf(" %02X", ((unsigned char *)want)[at]);
    printf("\n");
}

void far probe_failed(const char *what, int lost) {
    failures++;
    printf("FAIL %s lost %04X\n", what, lost);
}

void far call_scalars(void);
void far call_aggregates(void);

#define EXTERN(n, T) extern T gv_##n, out_##n; extern int gh_##n, gt_##n;
#define EXTERN_WIDE(n, T) extern long gw_##n, ow_##n;
SCALARS(EXTERN) NARROW(EXTERN_WIDE)
AGGREGATES(EXTERN)
extern signed char gm_a; extern long gm_b; extern double gm_c; extern int gm_d;
extern float gm_e; extern char far *gm_f; extern long double gm_h;

#define SAME(what, got, want) compare(what, &(got), &(want), sizeof(want))
#define CHECK(n, T) SAME("p_" #n, gv_##n, in_##n); SAME("r_" #n, out_##n, in_##n); \
    compare("h_" #n, &gh_##n, &head, 2); compare("t_" #n, &gt_##n, &tail, 2);
#define CHECK_WIDE(n, T) wide = in_##n; SAME("w_" #n, gw_##n, wide); SAME("rw_" #n, ow_##n, wide);

#ifdef VARIADIC
#define VEXTERN(n, T, P) extern P gva_##n;
PROMOTED(VEXTERN)
extern int gva_n;
#endif

int main(void) {
    int head = 0x1111, tail = 0x2222;
    long wide;
    call_scalars();
    call_aggregates();
    SCALARS(CHECK)
    NARROW(CHECK_WIDE)
    AGGREGATES(CHECK)
    SAME("m_a", gm_a, in_sc); SAME("m_b", gm_b, in_l); SAME("m_c", gm_c, in_d); SAME("m_d", gm_d, in_i);
    SAME("m_e", gm_e, in_f); SAME("m_f", gm_f, in_fp); SAME("m_h", gm_h, in_ld);
#ifdef VARIADIC
    compare("va_n", &gva_n, &head, 2);
    {
#define VCHECK(n, T, P) { P want = in_##n; SAME("va_" #n, gva_##n, want); }
        PROMOTED(VCHECK)
    }
#endif
    printf("%d failures\n", failures);
    return failures != 0;
}
