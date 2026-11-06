/* cc65 defines punct as not (space | control | digit | alpha), so bytes 128..255 count. */
int k_ispunct(int c)
{
    return c > 32 && c <= 255 && c != 127 && !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z'));
}
