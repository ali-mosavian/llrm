name: tosdivax
hand: tosdivax
kernel: k_tosdivax
args: W(60707), W(37)
wide: 1
rel: -
expect: 65406
note: signed 16-bit divide a/b, truncating (a on the C stack, b in A/X); a=-4829, b=37; b=0 and $8000 operands not checked
