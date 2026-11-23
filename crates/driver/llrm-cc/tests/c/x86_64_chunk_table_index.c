void *chunk(void **table, unsigned id) { return table[id >> 10]; }
unsigned slot(unsigned long long *table, unsigned id) { return (unsigned)table[id >> 10]; }
unsigned word(unsigned *table, unsigned id) { return table[id >> 10]; }
unsigned short half(unsigned short *table, unsigned id) { return table[id >> 10]; }
unsigned char byte(unsigned char *table, unsigned id) { return table[id >> 10]; }
void put(unsigned *table, unsigned id, unsigned v) { table[id >> 10] = v; }
void *wide(void **table, unsigned long long id) { return table[id >> 10]; }
