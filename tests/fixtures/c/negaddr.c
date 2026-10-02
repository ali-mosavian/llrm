/* Two shapes QCport's dl.c and host.c have that the HIR verifier refused
   once the driver ran it: a signed negation, which states no signed wrap,
   and a function's address taken in code. */
extern void install(void (*handler)(void));
static void tick(void) { }

int negated(int x) { return -x; }
void attach(void) { install(tick); }
