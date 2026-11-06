name: strcspn
hand: _strcspn
kernel: k_strcspn
args: P(mix,0),P(set,0)
wide: 1
rel: -
expect: 2
note: length of the initial run of s free of bytes in set; hand ignores the high byte of strlen(set), so sets over 255 bytes are unsupported
deps: strlen      # linked with it: the installed strlen does not leave ptr4 alone
