int k(const unsigned char *p, int n) {
    unsigned crc = 0xFFFF;
    while (n--) {
        crc ^= (unsigned)*p++ << 8;
        for (int i = 0; i < 8; i++) crc = (crc & 0x8000) ? (crc << 1) ^ 0x1021 : crc << 1;
        crc &= 0xFFFF;
    }
    return (crc >> 8) ^ (crc & 0xFF);
}
