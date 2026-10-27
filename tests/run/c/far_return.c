// flags: -O0 | -O2 -march=i486
// targets: x86-m16
/* A far pointer returned in AX:DX, picked from two arguments or moved by one element, under --isel=dag. */
extern void report(long v);
short __far cells[4] = {11, 22, 33, 44};
short __far *pick(short __far *a, short __far *b, int first) { return first ? a : b; }
short __far *step(short __far *p) { return p + 1; }
int main(void) {
    short __far *p = pick(&cells[1], &cells[3], 0);
    report(*p);
    report(*step(&cells[0]));
    return 0;
}
