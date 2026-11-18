int lead0q(unsigned long x) { return x ? __builtin_clzl(x) : 64; }
