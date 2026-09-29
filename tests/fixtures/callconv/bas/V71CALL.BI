    dim byvH as integer, byvL as long
    byvH = &H1111: byvL = -&H12345679
    call arm: pByv byvH, byvL, 1.5, -2.25, &H2222: verify 44
    putI 97, byvH: putL 98, byvL
