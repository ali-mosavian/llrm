/* __udivdi3. */
#include "i64.h"

u64 SFB_NAME(udivdi3)(u64 n, u64 d) { pair r; return join(udivmod(split(n), split(d), &r)); }
