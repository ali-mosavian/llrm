name: strncmp
hand: _strncmp
kernel: k_strncmp
args: P(mix,0), P(mix,0), W(20)
wide: 1
rel: -
expect: 0
note: equal strings: result 0 exactly; on a difference only the sign is defined (A is a byte of s1)
