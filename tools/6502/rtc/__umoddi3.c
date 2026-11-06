#include "_di.h"
extern u64 __udivmoddi4 (u64, u64, u64 *);
u64 __umoddi3 (u64 a, u64 b) { u64 r; __udivmoddi4 (a, b, &r); return r; }
