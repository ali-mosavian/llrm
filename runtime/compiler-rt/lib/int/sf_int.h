/* Integer builtins. SFB_NAME(n) is `__n` in the runtime, `sfb_n` in the host test.
 * Division by zero: quotient all ones, remainder the dividend. INT_MIN / -1 is INT_MIN, remainder 0.
 * No body uses a 64-bit operation: the 64-bit types are split into 32-bit halves. */
#ifndef SF_INT_H
#define SF_INT_H

#include <stdint.h>

#ifndef SFB_NAME
#define SFB_CAT(a, b) a##b
#define SFB_NAME(n) SFB_CAT(__, n)
#endif

#endif
