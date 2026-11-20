#include <string.h>
typedef unsigned char u8;
typedef unsigned u32;
extern void sink(u8 *, u32);
extern u8 *at(u32);

static void justify(u8 *const dst, const u32 len, const u8 *const src, const u32 count, const _Bool right)
{
    const u32 n = count < len ? count : len;
    const u32 before = right ? len - n : 0;
    memset(dst, ' ', before);
    memmove(dst + before, src, n);
    memset(dst + before + n, ' ', len - n - before);
}

struct S {
    u8 *recs;
    u32 nrec;
    u8 buf[80];
    u32 *sp;
};

void set(struct S *S, _Bool right, const u8 *src, u32 count, u32 kind, u32 addr)
{
    if ((kind >> 24) != 1) {
        justify(at(addr), kind & 0xffff, src, count, right);
    } else {
        u32 *d = (u32 *)at(addr);
        if (d[0] >= (u32)(size_t)S->recs && d[0] < (u32)(size_t)S->recs + S->nrec * 128) {
            justify(at(d[0]), (u32)d[1], src, count, right);
        } else {
            u32 len = d[1];
            justify(S->buf, len, src, count, right);
            sink(S->buf, len);
        }
    }
    S->sp += 3;
}
