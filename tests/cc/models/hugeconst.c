/* 6000:0000 + 70000 is 7000:1170: a carry past the offset is 0x1000 in the selector. */
unsigned char huge *at70000(void) { return (unsigned char huge *)0x60000000L + 70000L; }
