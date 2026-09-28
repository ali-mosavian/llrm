/* Reduced from qcport's sys.c and item.c: code addresses in static data. */
typedef short (near *Take)( short value );
typedef void (*Notice)( short value );

static short near twice( short value ) { return value + value; }
static short shown;
static void show( short value ) { shown = value; }

static const Take takes[] = { twice };
Notice notices[] = { show };

short run( short value )
{
    notices[0]( value );
    return takes[0]( value );
}
