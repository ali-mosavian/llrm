/* The compares are unsigned so the else branches know 0 <= n < 32 (x86 matches shld: ashldi3 52 -> 41 B at m32). __ashldi3 with libgcc's and compiler-rt's contract: a count of 0..63 (64 or more is undefined, as the C shift it implements; the guard that returned 0 cost 24 B per shift on m68k). */
#include "i64.h"

u64 SFB_NAME(ashldi3)(u64 x, int n)
{
    pair a = split(x), r = { 0, 0 };
    if ((unsigned)n >= 32) r.hi = a.lo << (n - 32);
    else if (n) { r.lo = a.lo << n; r.hi = (a.hi << n) | (a.lo >> (32 - n)); }
    else r = a;
    return join(r);
}
