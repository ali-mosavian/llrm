/* The local pointer has one exact frame-object target throughout this body. */
typedef struct { short x; short y; short z; } Trio;

short copyPointerSum( void )
{
    Trio source;
    Trio local;
    Trio *pointer;

    source.x = 11;
    source.y = 13;
    source.z = 17;
    pointer = &source;
    local = *pointer;
    return local.x + local.y + local.z;
}
