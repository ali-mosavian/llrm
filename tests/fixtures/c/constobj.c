/* A const object nothing writes: writing it is undefined, wherever its
   address went. */
extern const int ek;
extern int ev;
const int tbl[4] = { 1, 2, 3, 4 };
extern void ext(void);
extern void keep(const int *);

int ext_const(void) { int a = ek; ext(); return a + ek; }
int ext_plain(void) { int a = ev; ext(); return a + ev; }
int table(int i) { int a = tbl[i]; ext(); return a + tbl[i]; }
int escaped(int i) { int a = tbl[i]; keep(tbl); return a + tbl[i]; }
int stored(int *p, int i) { int a = tbl[i]; *p = 1; return a + tbl[i]; }
