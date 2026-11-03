/* __fixdfsi, __fixunsdfsi, __fixdfdi, __fixunsdfdi. */
#include "conv.h"

int32_t SF_NAME(fixdfsi)(sf_double a) { return (int32_t)to_signed(d2b(a), 32); }
uint32_t SF_NAME(fixunsdfsi)(sf_double a) { return (uint32_t)to_unsigned(d2b(a), 32); }
int64_t SF_NAME(fixdfdi)(sf_double a) { return to_signed(d2b(a), 64); }
uint64_t SF_NAME(fixunsdfdi)(sf_double a) { return to_unsigned(d2b(a), 64); }

#include "../core_end.inc"
