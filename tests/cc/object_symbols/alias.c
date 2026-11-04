int impl(int x) { return x + 1; }
int other(int x) __attribute__((alias("impl")));
__attribute__((visibility("hidden"))) int secret(int x) { return x * 3; }
