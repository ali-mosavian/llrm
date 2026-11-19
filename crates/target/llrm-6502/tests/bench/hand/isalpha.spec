name: isalpha
hand: _isalpha
kernel: k_isalpha
args: W(65)
wide: 1
rel: -
expect: 1
hand_expect: 2
note: isalpha('A'); hand returns the class mask (1 lower, 2 upper), kernel 0/1; check compares truthiness
