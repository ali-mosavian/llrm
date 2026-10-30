; A dword divisor (not negative): only the dividend's sign matters; ebp holds its sign mask, 0 or -1.
    push ebp
    mov ebp, edx
    add ebp, ebp
    sbb ebp, ebp
    xor eax, ebp
    xor edx, ebp
    sub eax, ebp
    sbb edx, ebp
    xor ecx, ecx
