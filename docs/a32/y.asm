; Can every 32-bit register be a base in real mode behind 67h?
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

    ; markers at 1000:0010 .. 1000:0017 = B0 .. B7
    mov ax, 0x1000
    mov es, ax
    mov cx, 8
    xor bx, bx
.w: mov al, 0xb0
    add al, bl
    mov [es:bx+0x10], al
    inc bx
    loop .w

%macro BASE 3                        ; %1 name, %2 reg, %3 want offset
    mov si, %1
    call puts
    mov byte [hit], 0
    mov word [back], %%d
    mov %2, %3
    mov al, [es:%2]
%%d: call verdict
%endmacro

    BASE nax, eax, 0x10
    BASE ncx, ecx, 0x11
    BASE ndx, edx, 0x12
    BASE nbx, ebx, 0x13
    ; esp as a base is legal too; keep interrupts off while it is not a stack
    mov si, nsp
    call puts
    mov byte [hit], 0
    mov word [back], .dsp
    cli
    mov ebp, esp
    mov esp, 0x14
    mov al, [es:esp]
    mov esp, ebp
    sti
.dsp: call verdict
    BASE nbp, ebp, 0x15
    BASE nsi, esi, 0x16
    BASE ndi, edi, 0x17

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
nax: db '[eax] ', 0
ncx: db '[ecx] ', 0
ndx: db '[edx] ', 0
nbx: db '[ebx] ', 0
nsp: db '[esp] ', 0
nbp: db '[ebp] ', 0
nsi: db '[esi] ', 0
ndi: db '[edi] ', 0
eq: db '= ', 0
exc: db 'EXC ', 0
nl: db 13, 10, 0
done: db 'end', 13, 10, 0
    times 510-($-$$) db 0
    dw 0xaa55
