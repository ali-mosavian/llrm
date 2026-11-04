; In: edx:eax / ecx:ebx, ecx != 0. Out: quotient edx:eax, remainder ecx:ebx. Keeps ebp, esi, edi.
; The quotient is under 2^32. Divide the dividend halved by the divisor's top dword, shifted so
; its top bit is set: that estimate is one over or exact; take one off, then add it back if the
; remainder it leaves is still no less than the divisor.
wide:
    cmp edx, ecx
    jae wide_go
    mov ebx, eax
    mov ecx, edx
    xor eax, eax
    xor edx, edx
    jmp done
wide_go:
    push ebp
    push esi
    push edi
    mov esi, eax
    mov edi, edx
    push ecx
    push ebx
    bsr edx, ecx
    xor edx, 31
    mov ebp, ecx
    mov ecx, edx
    shld ebp, ebx, cl
    shl ebx, cl
    mov ecx, ebp
    mov ebp, edx
    mov eax, esi
    mov edx, edi
    shr edx, 1
    rcr eax, 1
    div ecx
    mov ecx, 31
    sub ecx, ebp
    shr eax, cl
    test eax, eax
    jz wide_exact
    dec eax
wide_exact:
    pop ebx
    pop ebp
    push eax
    mov ecx, ebp
    imul ecx, eax
    mul ebx
    add edx, ecx
    sub esi, eax
    sbb edi, edx
    mov ecx, edi
    cmp esi, ebx
    sbb ecx, ebp
    pop eax
    jb wide_out
    sub esi, ebx
    sbb edi, ebp
    inc eax
wide_out:
    mov ebx, esi
    mov ecx, edi
    xor edx, edx
    pop edi
    pop esi
    pop ebp
