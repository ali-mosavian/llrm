extern short twice( short n );
#pragma aux twice parm [ax] value [ax];

short caller( short n )
{
    return twice( n );
}
