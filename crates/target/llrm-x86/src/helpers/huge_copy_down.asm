; A copy between huge pointers, downward (a memmove whose destination is above its source): the same operands as
; huge_copy, copied from the last byte, so that no byte is written before it is read. Each run ends where either
; offset wraps below 0, that selector stepped to the previous 64K. ds is kept.
; Changes eax, ebx, ecx, edx, si, di, es.
    push ds
    mov edx, ecx
    dec edx
    push ecx
    xor ecx, ecx
    mov cx, di
    add ecx, edx
    mov di, cx
    shr ecx, 16
    shl cx, 12
    add bx, cx
    xor ecx, ecx
    mov cx, si
    add ecx, edx
    mov si, cx
    shr ecx, 16
    shl cx, 12
    add ax, cx
    pop ecx
    mov es, bx
    mov ds, ax
    std
next:
    test ecx, ecx
    je done
    xor edx, edx
    mov dx, si
    inc edx
    xor ebx, ebx
    mov bx, di
    inc ebx
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
    sub si, 3
    sub di, 3
    rep movsd
    add si, 3
    add di, 3
    mov ecx, ebx
    and ecx, 3
    rep movsb
    pop ecx
    cmp si, 0xFFFF
    jne source_kept
    mov ax, ds
    sub ax, 0x1000
    mov ds, ax
source_kept:
    cmp di, 0xFFFF
    jne next
    mov ax, es
    sub ax, 0x1000
    mov es, ax
    jmp next
done:
    cld
    pop ds
