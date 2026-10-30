; A copy between huge pointers, upward: ds:si to es:di for `ecx` bytes, each run ending where either offset wraps,
; that selector stepped to the next 64K (4096 paragraphs). In: ecx = byte count, ax = source selector, si = source
; offset, bx = destination selector, di = destination offset. Out: nothing. ds is kept.
; Changes eax, ebx, ecx, edx, si, di, es.
    push ds
    mov es, bx
    mov ds, ax
    cld
next:
    test ecx, ecx
    je done
    xor edx, edx
    mov dx, si
    neg edx
    add edx, 0x10000
    xor ebx, ebx
    mov bx, di
    neg ebx
    add ebx, 0x10000
    cmp ebx, edx
    jae nearer
    mov edx, ebx
nearer:
    cmp ecx, edx
    jae whole
    mov edx, ecx
whole:
    sub ecx, edx
    push ecx
    mov ebx, edx
    mov ecx, edx
    shr ecx, 2
    rep movsd
    mov ecx, ebx
    and ecx, 3
    rep movsb
    pop ecx
    test si, si
    jne source_kept
    mov ax, ds
    add ax, 0x1000
    mov ds, ax
source_kept:
    test di, di
    jne next
    mov ax, es
    add ax, 0x1000
    mov es, ax
    jmp next
done:
    pop ds
