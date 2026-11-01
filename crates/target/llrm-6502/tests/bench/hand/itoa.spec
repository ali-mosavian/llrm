name: itoa
hand: _itoa
kernel: k_itoa
args: W(60707), P(dst,0), W(10)
wide: 1
rel: dst
expect: 0
note: itoa(-4829, dst, 10) writes "-4829" to dst and returns dst; the written string and the bytes after it are checked in itoa.check.c
