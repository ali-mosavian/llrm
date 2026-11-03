name: strstr
hand: _strstr
kernel: k_strstr
args: P(str,0),P(pat,0)
wide: 1
rel: str
expect: 4
note: first occurrence of needle in haystack (naive); empty needle returns the haystack; NULL if absent
