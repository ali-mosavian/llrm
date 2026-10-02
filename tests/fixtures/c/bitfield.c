/* Bit fields as Borland C 3.1 lays them out: a stream of bits, a field at
   the next byte when it would leave the 16 bits from its byte (bcc -S). */
struct flags {
    unsigned a : 3;
    int b : 5;
    unsigned c : 9;
    unsigned char d;
    unsigned e : 1;
};

/* 0 when every field reads back and the bytes are Borland's. */
int check(void)
{
    struct flags f;
    unsigned char *bytes = (unsigned char *)&f;
    int failed = 0, i;
    for (i = 0; i < 5; i++)
        bytes[i] = 0;
    f.a = 5;
    f.b = -3;
    f.c = 300;
    f.d = 0x77;
    f.e = 1;
    if (sizeof f != 5) failed |= 1;
    if (bytes[0] != 0xED || bytes[1] != 0x2C || bytes[2] != 0x01 || bytes[3] != 0x77 || bytes[4] != 0x01) failed |= 2;
    if (f.a != 5 || f.b != -3 || f.c != 300 || f.e != 1) failed |= 4;
    f.a++;
    f.b--;
    if (f.a != 6 || f.b != -4 || f.c != 300) failed |= 8;
    if ((f.c = 513) != 1) failed |= 16;
    return failed;
}
