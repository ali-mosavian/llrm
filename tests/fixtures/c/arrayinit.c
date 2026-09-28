/* Reduced from qcport's sc.c: a local array initialized from a constant. */
short pick( short at )
{
    short links[5] = { 0x101, 0x201, 0x401, 7, 1793 };
    return links[at];
}
