; 16-bit EA wraps at 64K. Does the 32-bit form wrap too?
bits 16
org 0x7c00
start:
    xor ax, ax
    mov ds, ax
    mov ss, ax
    mov sp, 0x7c00
    cld
    mov word [0x06*4], ud
    mov word [0x06*4+2], 0
    mov word [0x0d*4], gp
    mov word [0x0d*4+2], 0

    mov ax, 0x2000
    mov es, ax
    mov byte [es:0x000f], 0x77       ; what a wrap would land on

    ; 16-bit: bx + si = 0x1000f, truncated to 0x000f
    mov si, w16
    call puts
    mov byte [hit], 0
    mov word [back], .d1
    mov bx, 0xffff
    mov si, 0x10
    mov al, [es:bx+si]
.d1: call verdict

    ; 32-bit: ebx + esi = 0x1000f, past the limit
    mov si, w32
    call puts
    mov byte [hit], 0
    mov word [back], .d2
    mov ebx, 0xffff
    mov esi, 0x10
    mov al, [es:ebx+esi]
.d2: call verdict

    mov si, done
    call puts
    cli
    hlt

ud: mov byte [cs:hit], 6
    jmp fix
gp: mov byte [cs:hit], 13
fix:
    mov bp, sp
    mov ax, [cs:back]
    mov [bp], ax
    iret
verdict:
    mov ah, [cs:hit]
    test ah, ah
    jnz .f
    push ax
    mov si, eq
    call puts
    pop ax
    call hex2
    jmp .n
.f: mov si, exc
    call puts
    mov al, ah
    call hex2
.n: mov si, nl
    call puts
    ret
hex2:
    mov ah, al
    shr al, 4
    call .n
    mov al, ah
.n: and al, 0x0f
    add al, '0'
    cmp al, '9'
    jbe .e
    add al, 7
.e: out 0xe9, al
    ret
puts:
    lodsb
    test al, al
    jz .z
    out 0xe9, al
    jmp puts
.z: ret
hit: db 0
back: dw 0
w16: db '16 [bx+si] ffff+10 ', 0
w32: db '32 [ebx+esi] ffff+10 ', 0
eq: db '= ', 0
exc: db 'EXC ', 0
nl: db 13, 10, 0
done: db 'end', 13, 10, 0
    times 510-($-$$) db 0
    dw 0xaa55
