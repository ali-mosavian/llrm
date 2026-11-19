; In: edx:eax / ebx (a dword divisor, ecx = 0). Out: quotient edx:eax, remainder ecx:ebx.
; The size form of narrow.asm: two divs always, without the test that spares one.
    mov ecx, eax
    mov eax, edx
    xor edx, edx
    div ebx
    xchg eax, ecx
    div ebx
    mov ebx, edx
    mov edx, ecx
    xor ecx, ecx
