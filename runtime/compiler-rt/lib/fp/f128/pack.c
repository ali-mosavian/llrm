/* Round to nearest even and pack a binary128: three guard bits, implicit one at bit 115; e may be out of range. */
#include "f128.h"

sf128 sf128_pack(sf128 sign, int e, sf128 m) { return pack_any_128(sign, e, m); }

#include "../core_end.inc"
