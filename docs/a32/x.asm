; 386 addressing forms + a segment base: does that reach the whole 1MB?
bits 16
org 0x7c00
start:
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    cld
    mov word [0x06*4], ud
    mov word [0x06*4+2], 0
    mov word [0x0d*4], gp
    mov word [0x0d*4+2], 0

    ; markers, written the plain 16-bit way
    mov ax, 0x1000
    mov es, ax
    mov byte [es:0], 0x11            ; 0x10000
    mov ax, 0x5000
    mov es, ax
    mov byte [es:0x1234], 0x22       ; 0x51234
    mov ax, 0x9000
    mov es, ax
    mov byte [es:0x8000], 0x33       ; 0x98000
    mov ax, 0xf000
    mov es, ax
    mov byte [es:0xffff], 0x44       ; 0xfffff -- ROM, must NOT take

    ; read them back with 32-bit forms through a segment
    mov si, t1
    call puts
    mov ax, 0x1000
    mov es, ax
    xor eax, eax
    mov byte [hit], 0
    mov word [back], .d1
    mov al, [es:eax]
.d1: call verdict                    ; want 11

    mov si, t2
    call puts
    mov ax, 0x5000
    mov es, ax
    xor eax, eax
    xor esi, esi
    mov byte [hit], 0
    mov word [back], .d2
    mov al, [es:eax+esi*4+0x1234]
.d2: call verdict                    ; want 22

    mov si, t3
    call puts
    mov ax, 0x5000
    mov es, ax
    xor eax, eax
    mov esi, 0x48d                   ; 0x48d*4 = 0x1234
    mov byte [hit], 0
    mov word [back], .d3
    mov al, [es:eax+esi*4]
.d3: call verdict                    ; want 22, by scaling alone

    mov si, t4
    call puts
    mov ax, 0x9000
    mov es, ax
    mov ebp, 0x8000
    xor edi, edi
    mov byte [hit], 0
    mov word [back], .d4
    mov al, [es:ebp+edi*8]
.d4: call verdict                    ; want 33

    mov si, t5
    call puts
    mov ax, 0xf000
    mov es, ax
    mov ebx, 0xffff
    mov byte [hit], 0
    mov word [back], .d5
    mov al, [es:ebx]
.d5: call verdict                    ; ROM: want 03, not 44

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

hit:  db 0
back: dw 0
t1: db '1000:[eax] ', 0
t2: db '5000:[eax+esi*4+1234] ', 0
t3: db '5000:[eax+esi*4] ', 0
t4: db '9000:[ebp+edi*8] ', 0
t5: db 'f000:[ebx] ', 0
eq: db '= ', 0
exc: db 'EXC ', 0
nl: db 13, 10, 0
done: db 'end', 13, 10, 0
    times 510-($-$$) db 0
    dw 0xaa55
