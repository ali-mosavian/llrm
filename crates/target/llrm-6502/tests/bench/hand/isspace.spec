name: isspace
hand: _isspace
kernel: k_isspace
args: W(32)
wide: 1
rel: -
expect: 1
hand_expect: 32
note: isspace(' '); hand returns the class mask (0x20 for space), kernel 0/1; check skips 127 (cc65 table flags DEL as white space)
