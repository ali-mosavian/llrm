; With a 0 selector, how far does a 32-bit offset reach in real mode?
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

    ; a byte we can recognise, at ds:0
    mov byte [0], 0x5a

%macro AT 2                      ; %1 label, %2 32-bit offset
    mov si, %1
    call puts
    mov byte [hit], 0
    mov word [back], %%d
    xor ecx, ecx
    mov al, [ecx*2+%2]
%%d: call verdict
%endmacro

    mov si, hdr1
    call puts
    AT o0,     0x00000            ; ds=0, offset 0
    AT offff,  0x0ffff            ; last byte of the segment
    AT o10000, 0x10000            ; one past it
    AT o80000, 0x80000            ; 512K, inside the 1MB
    AT offfff, 0xfffff            ; the top of the 1MB

    ; the real-mode way to the same 512K byte: 8000:0000
    mov si, hdr2
    call puts
    mov si, seg8000
    call puts
    mov byte [hit], 0
    mov word [back], .d6
    mov ax, 0x8000
    mov es, ax
    mov al, [es:0]
.d6: call verdict

    ; top of the 1MB, f000:ffff = 0xfffff
    mov si, segf000
    call puts
    mov byte [hit], 0
    mov word [back], .d7
    mov ax, 0xf000
    mov es, ax
    mov al, [es:0xffff]
.d7: call verdict

    ; ffff:0010 is 0x100000, past the 1MB. Show the byte, not just "ok":
    ; 0x5a means it wrapped to 0, anything else means A20 let it through.
    mov si, wrap
    call puts
    mov byte [hit], 0
    mov word [back], .d8
    mov ax, 0xffff
    mov es, ax
    mov al, [es:0x10]
.d8: call verdict
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

verdict:                          ; "= xx" on success, "EXC nn" on a fault
    mov ah, [hit]
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
hdr1: db 'ds=0+67h', 13, 10, 0
hdr2: db 'seg', 13, 10, 0
o0:      db ' [0] ', 0
offff:   db ' [ffff] ', 0
o10000:  db ' [10000] ', 0
o80000:  db ' [80000] ', 0
offfff:  db ' [fffff] ', 0
seg8000: db ' 8000:0 ', 0
segf000: db ' f000:ffff ', 0
wrap:    db ' ffff:10 ', 0
eq:   db '= ', 0
exc:  db 'EXC ', 0
nl:   db 13, 10, 0
done: db 'end', 13, 10, 0
    times 510-($-$$) db 0
    dw 0xaa55
