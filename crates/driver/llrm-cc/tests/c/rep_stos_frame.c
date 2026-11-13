/* A frame object filled with a repeated byte, 700 B at 8-byte alignment. */
void fill_frame(unsigned char *out) {
    unsigned char b[700];
    __builtin_memset(b, 0xEE, 700);
    for (int i = 0; i < 700; i++) out[i] ^= b[i];
}
