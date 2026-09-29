    dim c as currency
    c = -1234.5678
    call arm: pCur &H1111, c, &H2222: verify 40
    putY 83, c
    call arm: putY 85, rCur@(&H3333): verify 41
