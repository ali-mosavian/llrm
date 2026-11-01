name: isdigit
hand: _isdigit
kernel: k_isdigit
args: W(53)
wide: 1
rel: -
expect: 1
hand_expect: 4
note: isdigit('5'); hand returns the class mask (4), kernel 0/1; check compares truthiness
