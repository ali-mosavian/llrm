/* Canonical CRC-32/ISO-HDLC check vector: "123456789" -> CBF43926. */
unsigned long bench_crc(unsigned long salt)
{
    static const unsigned char data[] = "123456789";
    unsigned long crc = 0xffffffffUL ^ salt;
    unsigned short i, bit;

    for (i = 0; i < 9; ++i) {
        crc ^= data[i];
        for (bit = 0; bit < 8; ++bit)
            crc = (crc >> 1) ^ (0xedb88320UL & (0UL - (crc & 1UL)));
    }
    return crc ^ 0xffffffffUL;
}
