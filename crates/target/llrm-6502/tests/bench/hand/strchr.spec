name: strchr
hand: _strchr
kernel: k_strchr
args: P(str,0),W(111)
wide: 1
rel: str
expect: 4
note: first occurrence of (char)c in s, the terminator included (c = 0 finds it); NULL if absent; only c's low byte counts
