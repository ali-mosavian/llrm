#ifndef STDINT_H
#define STDINT_H
typedef unsigned char uint8_t; typedef signed char int8_t;
typedef unsigned int uint16_t; typedef int int16_t;
typedef unsigned long uint32_t; typedef long int32_t;
typedef unsigned long long uint64_t; typedef long long int64_t;

#define UINT32_C(x) x##UL
#define UINT64_C(x) x##ULL
#define INT32_MAX 2147483647L
#define INT32_MIN (-2147483647L-1)
#define UINT32_MAX 4294967295UL
#define INT64_MAX 9223372036854775807LL
#define INT64_MIN (-9223372036854775807LL-1)
#define UINT64_MAX 18446744073709551615ULL
#endif
