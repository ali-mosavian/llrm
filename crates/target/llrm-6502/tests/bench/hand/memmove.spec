name: memmove
hand: _memmove
kernel: k_memmove
args: P(dst,0), P(str,0), W(8)
wide: 1
rel: dst
expect: 0
note: overlap-safe copy, returns dst; disjoint args, so the direction taken is set by link order of dst and str
