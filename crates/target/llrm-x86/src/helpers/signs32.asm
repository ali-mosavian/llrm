; A dword divisor (not negative): only the dividend's sign matters; ebp holds 1 if it was negative.
    push ebp
    xor ebp, ebp
    test edx, edx
    jns signs32_done
    neg edx
    neg eax
    sbb edx, 0
    mov ebp, 1
signs32_done:
    xor ecx, ecx
