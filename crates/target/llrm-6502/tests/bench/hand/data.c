/* The bench driver's data, as C, for the equivalence checks. Keep in step with `driver` in
   ../../bench.rs. A check program that modifies one of these works on a fresh copy. */
unsigned char buf[16] = {3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5, 8, 9, 7, 9, 3};
char str[] = "hello world";
unsigned char tab[8] = {2, 5, 8, 11, 14, 17, 20, 23};
char mix[] = "Hello, World 42!";
char mix2[] = "hELLO, wORLD 42?";
char num[] = "  -4829xyz";
char pat[] = "o w";
char set[] = "lo";
char dst[64];
