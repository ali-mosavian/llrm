name: strpbrk
hand: _strpbrk
kernel: k_strpbrk
args: P(str,0),P(set,0)
wide: 1
rel: str
expect: 2
note: pointer to the first byte of s that is in set (the terminator never matches), NULL if none
