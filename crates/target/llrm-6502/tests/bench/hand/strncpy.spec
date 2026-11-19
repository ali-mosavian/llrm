name: strncpy
hand: _strncpy
kernel: k_strncpy
args: P(dst,0),P(str,0),W(20)
wide: 1
rel: dst
expect: 0
note: copies at most n bytes of src, zero-pads dst up to n (no terminator when src is n or longer); returns dst; idempotent
