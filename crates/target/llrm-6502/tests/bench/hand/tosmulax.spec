name: tosmulax
hand: tosmulax
kernel: k_tosmulax
args: W(1234), W(567)
wide: 1
rel: -
expect: 44318
note: signed 16x16 multiply, first operand on the C stack, second in A/X, result modulo 65536; shares tosmulax.s with tosumulax
