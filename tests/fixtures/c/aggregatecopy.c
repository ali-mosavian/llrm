/* A packed three-word copy forces one dword move to overlap two scalar leaves. */
typedef struct { short x; short y; short z; } Trio;

Trio source;

short copySum( void )
{
    Trio local;

    local = source;
    return local.x + local.y + local.z;
}
