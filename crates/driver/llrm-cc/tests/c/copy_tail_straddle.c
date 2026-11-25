/* A 6-byte copy whose reader loads 2 bytes at 0 and 4 bytes at 2. */
struct __attribute__((packed)) pair {
    __UINT16_TYPE__ len;
    __UINT32_TYPE__ ptr;
};

void get(struct pair *out, const char *from)
{
    __builtin_memcpy(out, from, 6);
}
