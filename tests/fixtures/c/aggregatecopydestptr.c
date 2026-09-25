/* The destination pointer has one exact frame-object target throughout this body. */
typedef struct { short x; short y; short z; } Trio;

Trio source;

short copyDestinationPointerSum( void )
{
    Trio local;
    Trio *destination;

    destination = &local;
    *destination = source;
    return local.x + local.y + local.z;
}
