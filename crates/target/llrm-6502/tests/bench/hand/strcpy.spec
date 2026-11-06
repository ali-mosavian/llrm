name: strcpy
hand: _strcpy
kernel: k_strcpy
args: P(dst,0),P(str,0)
wide: 1
rel: dst
expect: 0
note: copies src with its terminator to dst, returns dst; idempotent (same bytes every call)
