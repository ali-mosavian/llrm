name: atoi
hand: _atoi
kernel: k_atoi
args: P(num,0)
wide: 1
rel: -
expect: 60707
note: atoi("  -4829xyz") = -4829; skips only space and tab (not \n,\v,\f,\r); wraps at 16 bits; 0xED23 is 16-bit two's complement
