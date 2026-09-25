/* A far pointer read from memory and followed: bcc's `les bx,[table]`. */
typedef struct { short x, y; } Pt;

Pt far *table;

short sum( void )
{
    return table->x + table->y;
}
