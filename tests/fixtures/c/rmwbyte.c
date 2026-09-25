/* A non-volatile far byte compound assignment must remain a byte RMW.
 *
 * BCC emits `or byte ptr es:[bx+di],al`.  Expanding it through promoted words
 * costs registers as well as instructions: that was what prevented the
 * allocator's ordinary regional split from keeping r_walk's pointer bases.
 */

void mark_bit( unsigned char far *map, unsigned short value )
{
    map[value >> 3] |= (unsigned char)(1U << (value & 7));
}
