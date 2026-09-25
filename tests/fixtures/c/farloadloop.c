/* r_walk's shape: two near owners repeatedly load far pointer fields.
   BCC selects each field load as LES before register pressure can split the
   owner address across the offset and selector words. */
typedef struct {
    char padding[38];
    unsigned short far *words;
} World;

typedef struct {
    char padding[1014];
    unsigned char far *flags;
} Renderer;

void mark(World *world, Renderer *rdr, short first, short last)
{
    short i;
    short value;

    for (i = first; i < last; ++i) {
        value = world->words[i];
        rdr->flags[value >> 3] |= (unsigned char)(1 << (value & 7));
    }
}
