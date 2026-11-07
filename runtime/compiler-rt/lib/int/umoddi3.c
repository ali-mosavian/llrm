/* __umoddi3. */
#include "i64.h"

u64 SFB_NAME(umoddi3)(u64 n, u64 d) { pair r; udivmod(split(n), split(d), &r); return join(r); }
