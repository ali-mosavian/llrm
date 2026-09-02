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
    mov byte [0], 0x5a

    ; 4GB limit into ds, then back to real mode and RELOAD ds with 0
    cli
    lgdt [gdtr]
    mov eax, cr0
    or al, 1
    mov cr0, eax
    jmp short $+2
    mov bx, 0x08
    mov ds, bx
    mov eax, cr0
    and al, 0xfe
    mov cr0, eax
    jmp short $+2
    xor ax, ax
    mov ds, ax                  ; a 0 selector, loaded in real mode
    sti

%macro AT 2
    mov si, %1
    call puts
    mov byte [hit], 0
    mov word [back], %%d
    xor ecx, ecx
    mov al, [ecx*2+%2]
%%d: call verdict
%endmacro

    AT o0,     0x00000
    AT o80000, 0x80000
    AT offfff, 0xfffff
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

align 8
gdt:  dq 0
      dw 0xffff, 0, 0x9200, 0x00cf
gdtr: dw 23
      dd gdt
hit:  db 0
back: dw 0
o0:      db 'ds=0 unreal [0] ', 0
o80000:  db ' [80000] ', 0
offfff:  db ' [fffff] ', 0
eq:   db '= ', 0
exc:  db 'EXC ', 0
nl:   db 13, 10, 0
done: db 'end', 13, 10, 0
    times 510-($-$$) db 0
    dw 0xaa55
