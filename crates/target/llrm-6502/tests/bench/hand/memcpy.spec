name: memcpy
hand: _memcpy
kernel: k_memcpy
args: P(dst,0), P(str,0), W(8)
wide: 1
rel: dst
expect: 0
note: copies 8 bytes str to dst, returns dst; call rewrites dst with the same bytes (idempotent)
