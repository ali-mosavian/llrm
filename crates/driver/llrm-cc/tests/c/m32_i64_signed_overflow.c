/* signed 64-bit add and subtract with overflow on a 32-bit target (overflow_builtins: xor/not/test/sets, +17 B each) */
int sadd(long long a, long long b, long long *r) { return __builtin_add_overflow(a, b, r); }
int ssub(long long a, long long b, long long *r) { return __builtin_sub_overflow(a, b, r); }
/* its flag read twice after other flag writers: the saturating add compiled to a refusal once the flag was the carry chain's */
long long sat(long long a, long long b) { return __builtin_elementwise_add_sat(a, b); }
long long usat(long long a, long long b) { return __builtin_elementwise_sub_sat(a, b); }
