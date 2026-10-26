/* The three conventions a name can be spelled for, and a call through a function pointer. */
int cdecl c_add(int a, int b) { return a + b; }
int pascal p_add(int a, int b) { return a + b; }
int (*pick)(int, int);
int call_through(int a, int b) { pick = c_add; return pick(a, b); }
