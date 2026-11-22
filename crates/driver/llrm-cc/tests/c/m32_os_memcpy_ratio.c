/* A constant 13-byte copy at -Os: four load and store pairs of absolute
 * addresses inline, or `mov edi; mov esi; mov ecx; rep movsb` (gcc's -Os,
 * MOVE_RATIO 3). */
extern char dst[64];
extern char src[64];

void copy_name(void)
{
    __builtin_memcpy(dst + 3, src, 13);
}
