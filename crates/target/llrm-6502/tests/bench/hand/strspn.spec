name: strspn
hand: _strspn
kernel: k_strspn
args: P(str,0),P(set,0)
wide: 1
rel: -
expect: 0
note: length of the initial run of s made of bytes in set; hand ignores the high byte of strlen(set), so sets over 255 bytes are unsupported
deps: strlen      # linked with it: the installed strlen does not leave ptr4 alone
