/* Round to nearest even and pack a binary64: three guard bits, implicit one at bit 55; e may be out of range. */
#include "f64.h"

uint64_t sf64_pack(uint64_t sign, int e, uint64_t m) { return pack_any_64(sign, e, m); }

#include "../core_end.inc"
