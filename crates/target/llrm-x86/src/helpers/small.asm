; In: edx:eax / ecx:ebx, ecx:ebx != 0. Out: quotient edx:eax, remainder ecx:ebx. Keeps ebp, esi, edi.
; One bit of the quotient a round: the size routine, for code that is built for bytes.
small:
    push ebp
    push esi
    push edi
    xor esi, esi
    xor edi, edi
    mov ebp, 64
small_bit:
    add eax, eax
    adc edx, edx
    adc edi, edi
    adc esi, esi
    jc small_take
    sub edi, ebx
    sbb esi, ecx
    jnc small_set
    add edi, ebx
    adc esi, ecx
    jmp small_next
small_take:
    sub edi, ebx
    sbb esi, ecx
small_set:
    inc eax
small_next:
    dec ebp
    jnz small_bit
    mov ebx, edi
    mov ecx, esi
    pop edi
    pop esi
    pop ebp
