/* Every internal call agrees on the argument, so the body can specialize. */
static short twice( short value )
{
    return value * 2;
}

short answer_from_argument( void )
{
    return twice( 21 );
}
