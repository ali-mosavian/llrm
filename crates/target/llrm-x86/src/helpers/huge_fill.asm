; A fill through a huge pointer: es:di gets `ecx` bytes of the pattern in eax (its bytes in turn, low first), each
; run ending where the offset wraps, the selector stepped to the next 64K (4096 paragraphs). The dwords go by
; `rep stosd`, the bytes after them one at a time with the pattern turned, so the next run goes on where this one left
; off. In: eax = the pattern's dword, ecx = byte count, bx = selector, di = offset.
; Out: nothing. Changes eax, ebx, ecx, edx, di, es.
    mov es, bx
    cld
next:
    test ecx, ecx
    je done
    xor edx, edx
    mov dx, di
    neg edx
    add edx, 0x10000
    cmp ecx, edx
    jae whole
    mov edx, ecx
whole:
    sub ecx, edx
    push ecx
    mov ebx, edx
    mov ecx, edx
    shr ecx, 2
    rep stosd
    mov ecx, ebx
    and ecx, 3
    je ended
tail:
    stosb
    ror eax, 8
    dec cx
    jne tail
ended:
    pop ecx
    test di, di
    jne next
    mov bx, es
    add bx, 0x1000
    mov es, bx
    jmp next
done:
