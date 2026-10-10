; In: edx:eax / ebx (a dword divisor, ecx = 0). Out: quotient edx:eax, remainder ecx:ebx.
; The high dword divides first when it is not below the divisor: two divs, else one.
    cmp edx, ebx
    jae narrow_two
    div ebx
    mov ebx, edx
    mov edx, ecx
    jmp done
narrow_two:
    mov ecx, eax
    mov eax, edx
    xor edx, edx
    div ebx
    xchg eax, ecx
    div ebx
    mov ebx, edx
    mov edx, ecx
    xor ecx, ecx
    jmp done
