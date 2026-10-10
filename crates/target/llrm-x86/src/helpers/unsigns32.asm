    test ebp, ebp
    jz unsigns32_done
    neg edx
    neg eax
    sbb edx, 0
    neg ecx
    neg ebx
    sbb ecx, 0
unsigns32_done:
    pop ebp
