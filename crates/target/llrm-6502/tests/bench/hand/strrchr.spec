name: strrchr
hand: _strrchr
kernel: k_strrchr
args: P(str,0),W(111)
wide: 1
rel: str
expect: 7
note: last occurrence of (char)c in s, NULL if absent; hand returns NULL for c == 0 (cc65 bug; docs and k_strrchr give the terminator), so args avoid c == 0
