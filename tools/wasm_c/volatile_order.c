/* Call arguments that are volatile loads are read left to right. */
volatile int g1, g2, g3;
volatile long long q1, q2;
volatile double d1;
extern int use3(int, int, int);
extern int use5(int, long long, double, int, long long);
extern int one(int);
int f3(void) { return use3(g1, g2, g3); }
int f5(void) { return use5(g1, q1, d1, g2, q2); }
int fnest(void) { return use3(g1, one(g2), g3); }
