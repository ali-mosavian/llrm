name: tosmodax
hand: tosmodax
kernel: k_tosmodax
args: W(60707), W(37)
wide: 1
rel: -
expect: 65517
note: signed 16-bit remainder a%b, sign of a (a on the C stack, b in A/X); a=-4829, b=37; b=0 and $8000 operands not checked
