name: strcmp
hand: _strcmp
kernel: k_strcmp
args: P(mix,0), P(mix,0)
wide: 1
rel: -
expect: 0
note: equal strings: result 0 exactly; on a difference A holds a byte of s1 and X is +-1, so only the sign is defined
