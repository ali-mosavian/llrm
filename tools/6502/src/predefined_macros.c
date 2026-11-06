// flags: -O0 | -O2
// GCC's predefined macros, which programs test and build their types from (gcc.c-torture: 73 programs refused on `__SIZE_TYPE__` alone,
// widechar-3 on `__BYTE_ORDER__`): each agrees with what the compiler does, on either target. Every line prints 1.
extern void report(long value);

typedef __SIZE_TYPE__ size_type;
typedef __UINT32_TYPE__ word32;
typedef __INT16_TYPE__ half;
typedef __UINT64_TYPE__ word64;

union order { unsigned short whole; unsigned char byte[2]; };

int main(void)
{
    union order probe;
    probe.whole = 0x0102;
    report(__SIZEOF_SHORT__ == sizeof(short) && __SIZEOF_INT__ == sizeof(int) && __SIZEOF_LONG__ == sizeof(long));
    report(__SIZEOF_LONG_LONG__ == sizeof(long long) && __SIZEOF_POINTER__ == sizeof(void *) && __SIZEOF_DOUBLE__ == sizeof(double));
    report(__INT_MAX__ == (int)(~0U >> 1) && __LONG_MAX__ == (long)(~0UL >> 1) && __SCHAR_MAX__ == 127 && __CHAR_BIT__ == 8);
    report(sizeof(size_type) == sizeof(sizeof(int)) && sizeof(word32) == 4 && sizeof(half) == 2 && sizeof(word64) == 8);
#if __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
    report(probe.byte[0] == 1);
#elif __BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__
    report(probe.byte[0] == 2);
#else
    report(0);
#endif
    return 0;
}
