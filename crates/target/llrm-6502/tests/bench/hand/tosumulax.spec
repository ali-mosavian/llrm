name: tosumulax
hand: tosumulax
kernel: k_tosumulax
args: W(1234), W(567)
wide: 1
rel: -
expect: 44318
note: unsigned 16x16 multiply, first operand on the C stack, second in A/X, result modulo 65536; entry in tosmulax.s
deps: tosmulax    # the entry lives in tosmulax.s
