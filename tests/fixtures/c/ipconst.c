/* Interprocedural constants: the caller cannot see this value locally. */
static short answer( void )
{
    return 37;
}

short add_answer( short value )
{
    return value + answer();
}
