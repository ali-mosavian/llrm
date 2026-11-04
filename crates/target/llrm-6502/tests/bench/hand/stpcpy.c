char *k_stpcpy(char *dst, const char *src)
{
    while ((*dst = *src++) != 0)
        dst++;
    return dst;
}
