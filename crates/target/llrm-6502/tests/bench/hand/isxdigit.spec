name: isxdigit
hand: _isxdigit
kernel: k_isxdigit
args: W(102)
wide: 1
rel: -
expect: 1
hand_expect: 8
note: isxdigit('f'); hand returns the class mask (8), kernel 0/1; check compares truthiness
