.model medium
.386

public N$OOPN
public N$OCRE
public N$OREA
public N$OWRI
public N$OCLO
public N$OEXT
public N$OMEM
public N$OGIV
public N$OSIV
public N$OCHN
public N$OVEC
public N$OTOP
public N$OPSP

.data
; The near heap's end, a DGROUP offset, and the program's PSP, whose
; memory block the heap grows. Startup sets both.
N$OTOP dw 0
N$OPSP dw 0

; Each vector the program replaced, and what it entered before, which
; N$OVEC puts back. A vector replaced past the last entry is not.
SAVED equ 16
saved_count dw 0
saved_number db SAVED dup (0)
saved_old dd SAVED dup (0)

.code
; N$OMEM(bytes: u16) -> *near mut u8: `bytes` more of DGROUP at the heap's end, or
; 0 when DGROUP's 64 KB or DOS's memory runs out.
N$OMEM proc far
    push bp
    mov bp, sp
    push bx
    push es
    mov bx, N$OTOP
    add bx, [bp+6]
    jc short refused
    cmp bx, 0fff0h
    ja short refused
    mov ax, bx
    add ax, 15
    shr ax, 4
    mov bx, DGROUP
    sub bx, N$OPSP
    add bx, ax
    mov es, N$OPSP
    mov ah, 4ah
    int 21h
    jc short refused
    mov ax, N$OTOP
    mov bx, [bp+6]
    add N$OTOP, bx
    jmp short grown
refused:
    xor ax, ax
grown:
    pop es
    pop bx
    pop bp
    retf
N$OMEM endp

; The DOS file calls. Each returns what DOS does, or when DOS sets carry,
; its error code negated.

; N$OOPN(name: *far char, mode: u8) -> i16: a handle to an existing file.
N$OOPN proc far
    push bp
    mov bp, sp
    push ds
    lds dx, [bp+6]
    mov al, [bp+10]
    mov ah, 3dh
    jmp short called
N$OOPN endp

; N$OCRE(name: *far char) -> i16: a handle to a new, empty file.
N$OCRE proc far
    push bp
    mov bp, sp
    push ds
    lds dx, [bp+6]
    xor cx, cx
    mov ah, 3ch
    jmp short called
N$OCRE endp

; N$OREA(handle: i16, data: *far mut u8, count: u16) -> i16: bytes read.
N$OREA proc far
    mov ah, 3fh
    jmp short transfer
N$OREA endp

; N$OWRI(handle: i16, data: *far u8, count: u16) -> i16: bytes written.
N$OWRI proc far
    mov ah, 40h
transfer::
    push bp
    mov bp, sp
    push ds
    push bx
    mov bx, [bp+6]
    lds dx, [bp+8]
    mov cx, [bp+12]
    int 21h
    pop bx
    jmp short checked
N$OWRI endp

; N$OCLO(handle: i16) -> i16
N$OCLO proc far
    push bp
    mov bp, sp
    push ds
    push bx
    mov bx, [bp+6]
    mov ah, 3eh
    int 21h
    pop bx
    jmp short checked
called::
    int 21h
checked::
    jnc short done
    neg ax
done:
    pop ds
    pop bp
    retf
N$OCLO endp

; Interrupt vectors, for handlers the program installs. A handler is
; entered with interrupts off and leaves by iret.

; N$OGIV(number: u8) -> extern "interrupt16" fn(): what interrupt `number`
; enters now.
N$OGIV proc far
    push bp
    mov bp, sp
    push bx
    push es
    mov al, [bp+6]
    mov ah, 35h
    int 21h
    mov ax, bx
    mov dx, es
    pop es
    pop bx
    pop bp
    retf
N$OGIV endp

; N$OSIV(number: u8, handler: extern "interrupt16" fn()): makes interrupt
; `number` enter `handler`, the first time keeping the one it replaces.
N$OSIV proc far
    push bp
    mov bp, sp
    push bx
    push si
    push es
    mov al, [bp+6]
    xor si, si
seek:
    cmp si, saved_count
    je short keep
    cmp saved_number[si], al
    je short install
    inc si
    jmp short seek
keep:
    cmp si, SAVED
    je short install
    mov saved_number[si], al
    mov ah, 35h
    int 21h
    shl si, 2
    mov word ptr saved_old[si], bx
    mov word ptr saved_old[si+2], es
    inc saved_count
install:
    push ds
    lds dx, [bp+8]
    mov al, [bp+6]
    mov ah, 25h
    int 21h
    pop ds
    pop es
    pop si
    pop bx
    pop bp
    retf
N$OSIV endp

; N$OVEC: puts back every vector N$OSIV replaced, the last first. Every
; exit path calls it.
N$OVEC proc far
    push ds
    push si
    mov ax, DGROUP
    mov ds, ax
    mov si, saved_count
    mov saved_count, 0
restore:
    dec si
    js short restored
    mov al, saved_number[si]
    push si
    shl si, 2
    push ds
    lds dx, saved_old[si]
    mov ah, 25h
    int 21h
    pop ds
    pop si
    jmp short restore
restored:
    pop si
    pop ds
    retf
N$OVEC endp

; N$OCHN(handler: extern "interrupt16" fn()): enters `handler` as its
; interrupt would, flags pushed, and comes back.
N$OCHN proc far
    push bp
    mov bp, sp
    pushf
    call dword ptr [bp+6]
    pop bp
    retf
N$OCHN endp

; N$OEXT(code: u8): restores the vectors and ends the program.
N$OEXT proc far
    push bp
    mov bp, sp
    call far ptr N$OVEC
    mov al, [bp+6]
    mov ah, 4ch
    int 21h
N$OEXT endp

end
