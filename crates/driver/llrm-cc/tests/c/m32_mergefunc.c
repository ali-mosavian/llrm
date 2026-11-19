/* two exported functions with the same body (gcc -fipa-icf folds them at -Os; sfm16 `__gtsf2`/`__gesf2`) */
int twin_a(int x) { return x * 7 + 3 - (x >> 2); }
int twin_b(int y) { return y * 7 + 3 - (y >> 2); }
/* a different body stays a body */
int other(int y) { return y * 7 + 4 - (y >> 2); }
