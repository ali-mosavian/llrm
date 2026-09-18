/* Both aggregate-copy addresses are exact local pointer targets. */
typedef struct { short x; short y; short z; } Trio;

short copyBothPointerSum( short x, short y, short z )
{
    Trio source;
    Trio local;
    Trio *from;
    Trio *to;

    source.x = x;
    source.y = y;
    source.z = z;
    from = &source;
    to = &local;
    *to = *from;
    return local.x + local.y + local.z;
}
