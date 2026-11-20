extern void *memcpy(void *, const void *, unsigned long);
extern void notify(void);

const unsigned char themes[4][48] = {{1}, {2}, {3}, {4}};
unsigned char attr[48];
unsigned char cur;

void set_theme(unsigned char n)
{
    cur = n;
    memcpy(attr, themes[n], 48);
    notify();
}

void dark(void) { set_theme(1); }
void light(void) { set_theme(2); }
void pick(unsigned char a) { set_theme(a + 3); }
