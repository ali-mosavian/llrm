/* An unbounded far source must remain one access: its leaves are not proven. */
typedef struct { short x; short y; short z; } Trio;

Trio far *source;

short copyFarSum( void )
{
    Trio local;

    local = *source;
    return local.x + local.y + local.z;
}
