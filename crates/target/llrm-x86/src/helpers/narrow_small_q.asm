; In: edx:eax / ebx (a dword divisor, ecx = 0). Out: quotient edx:eax; the remainder is not kept.
    mov ecx, eax
    mov eax, edx
    xor edx, edx
    div ebx
    xchg eax, ecx
    div ebx
    mov edx, ecx
