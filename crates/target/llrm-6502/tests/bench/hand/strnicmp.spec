name: strnicmp
hand: _strnicmp
kernel: k_strnicmp
args: P(mix,0),P(mix2,0),W(15)
wide: 1
rel: -
expect: 0
note: case-insensitive compare of at most n bytes (upper-folding, so '_' > 'a'); on a difference only the sign in X is defined
