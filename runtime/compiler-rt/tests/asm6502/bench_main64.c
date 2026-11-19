/* bench_f64.py driver: calls OP over the records start_idx .. start_idx+count_n-1 of V (a, b, expected; hi,lo each) and exits with the
 * number of wrong results. start_idx and count_n are patched into the binary, one run per record. */
#include <stdint.h>
#include "bench_vec.h"

uint64_t OP(uint64_t, uint64_t);
typedef union { uint32_t w[2]; uint64_t q; } D;

unsigned start_idx = 1, count_n = 1; /* initialised: they live in the file image, where the script patches them */

static int nan64(uint32_t hi, uint32_t lo) { return (hi & 0x7FF00000u) == 0x7FF00000u && ((hi & 0xFFFFFu) | lo); }

int main(void)
{
    unsigned bad = 0;
    for (unsigned i = start_idx; i < start_idx + count_n; i++) {
        const uint32_t *v = V[i];
        D x, y, r;
        x.w[0] = v[1]; x.w[1] = v[0]; y.w[0] = v[3]; y.w[1] = v[2];
        r.q = OP(x.q, y.q);
        if (!((r.w[1] == v[4] && r.w[0] == v[5]) || (nan64(r.w[1], r.w[0]) && nan64(v[4], v[5])))) bad++;
    }
    return bad > 255 ? 255 : bad;
}
