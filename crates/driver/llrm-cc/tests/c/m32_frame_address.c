extern void report(long v);
__attribute__((noinline)) static long fa(void) { return (long)__builtin_frame_address(0) != 0; }
int main(void) { report(fa()); return 0; }
