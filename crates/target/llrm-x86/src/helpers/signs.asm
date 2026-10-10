; Both operands signed: ebp holds bit 1 = the quotient is negative, bit 0 = the remainder is.
    push ebp
    xor ebp, ebp
    test edx, edx
    jns signs_divisor
    neg edx
    neg eax
    sbb edx, 0
    mov ebp, 3
signs_divisor:
    test ecx, ecx
    jns signs_done
    neg ecx
    neg ebx
    sbb ecx, 0
    xor ebp, 2
signs_done:
