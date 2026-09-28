/* Reduced from qcport's sys_time.c: the BIOS tick count at 0040:006C. */
unsigned long ticks( void )
{
    return *(unsigned long far *) ( ((unsigned long) 0x0040 << 16) | 0x006CUL );
}
