name: memset
hand: _memset
kernel: k_memset
args: P(dst,0), W(120), W(32)
wide: 1
rel: dst
expect: 0
note: fills 32 bytes of dst with the low byte of c, returns dst; idempotent
