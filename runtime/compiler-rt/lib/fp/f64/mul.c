/* __muldf3. */
#include "f64.h"
#include "mul64.h"

static inline sf64 mul_core(uint64_t sign, int ea, int eb, uint64_t ma, uint64_t mb)
{
    uint64_t hi, lo;
    mul64(ma, mb, &hi, &lo);
    /* The product has its top bit at 105 or 104; keep 56 bits and fold the rest. */
    int e = ea + eb - 1023, sh = 49;
    if (hi >> 41) { sh = 50; e++; }
    uint64_t m = (hi << (64 - sh)) | (lo >> sh) | ((lo << (64 - sh)) != 0);
    return SF_PACK(sign, e, m);
}

#include "../mul.inc"

sf_double SF_NAME(muldf3)(sf_double a, sf_double b) { return b2d(mul_bits_64(d2b(a), d2b(b))); }

#include "../core_end.inc"
