    test ebp, 2
    jz unsigns_remainder
    neg edx
    neg eax
    sbb edx, 0
unsigns_remainder:
    test ebp, 1
    jz unsigns_done
    neg ecx
    neg ebx
    sbb ecx, 0
unsigns_done:
    pop ebp
