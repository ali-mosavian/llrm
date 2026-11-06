name: memchr
hand: _memchr
kernel: k_memchr
args: P(str,0), W(119), W(11)
wide: 1
rel: str
expect: 6
note: pointer to the first byte equal to the low byte of c within n, or 0; finds 'w'
