name: memcmp
hand: _memcmp
kernel: k_memcmp
args: P(buf,0), P(buf,0), W(16)
wide: 1
rel: -
expect: 0
note: equal inputs: result 0 exactly; on a difference the hand routine leaves a byte of p1 in A and +-1 in X, so only the sign is defined
