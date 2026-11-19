name: ispunct
hand: _ispunct
kernel: k_ispunct
args: W(33)
wide: 1
rel: -
expect: 1
hand_expect: 255
note: ispunct('!'); hand returns 0xFF, kernel 0/1; cc65 counts bytes 128..255 as punct; check skips 127 (table flags DEL as white space)
