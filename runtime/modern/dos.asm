.model medium
.386

public M$OOPN
public M$OCRE
public M$OREA
public M$OWRI
public M$OCLO
public M$OEXT
public M$OMEM
public M$OGIV
public M$OSIV
public M$OCHN
public M$OVEC
public M$OTOP
public M$OPSP

.data
; The near heap's end, a DGROUP offset, and the program's PSP, whose
; memory block the heap grows. Startup sets both.
M$OTOP dw 0
M$OPSP dw 0

; Each vector the program replaced, and what it entered before, which
; M$OVEC puts back. A vector replaced past the last entry is not.
SAVED equ 16
saved_count dw 0
saved_number db SAVED dup (0)
saved_old dd SAVED dup (0)

.code
; M$OMEM(bytes: u16) -> *near mut u8: `bytes` more of DGROUP at the heap's end, or
; 0 when DGROUP's 64 KB or DOS's memory runs out.
M$OMEM proc far
    push bp
    mov bp, sp
    push bx
    push es
    mov bx, M$OTOP
    add bx, [bp+6]
    jc short refused
    cmp bx, 0fff0h
    ja short refused
    mov ax, bx
    add ax, 15
    shr ax, 4
    mov bx, DGROUP
    sub bx, M$OPSP
    add bx, ax
    mov es, M$OPSP
    mov ah, 4ah
    int 21h
    jc short refused
    mov ax, M$OTOP
    mov bx, [bp+6]
    add M$OTOP, bx
    jmp short grown
refused:
    xor ax, ax
grown:
    pop es
    pop bx
    pop bp
    retf
M$OMEM endp

; The DOS file calls. Each returns what DOS does, or when DOS sets carry,
; its error code negated.

; M$OOPN(name: *far char, mode: u8) -> i16: a handle to an existing file.
M$OOPN proc far
    push bp
    mov bp, sp
    push ds
    lds dx, [bp+6]
    mov al, [bp+10]
    mov ah, 3dh
    jmp short called
M$OOPN endp

; M$OCRE(name: *far char) -> i16: a handle to a new, empty file.
M$OCRE proc far
    push bp
    mov bp, sp
    push ds
    lds dx, [bp+6]
    xor cx, cx
    mov ah, 3ch
    jmp short called
M$OCRE endp

; M$OREA(handle: i16, data: *far mut u8, count: u16) -> i16: bytes read.
M$OREA proc far
    mov ah, 3fh
    jmp short transfer
M$OREA endp

; M$OWRI(handle: i16, data: *far u8, count: u16) -> i16: bytes written.
M$OWRI proc far
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
M$OWRI endp

; M$OCLO(handle: i16) -> i16
M$OCLO proc far
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
M$OCLO endp

; Interrupt vectors, for handlers the program installs. A handler is
; entered with interrupts off and leaves by iret.

; M$OGIV(number: u8) -> extern "interrupt16" fn(): what interrupt `number`
; enters now.
M$OGIV proc far
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
M$OGIV endp

; M$OSIV(number: u8, handler: extern "interrupt16" fn()): makes interrupt
; `number` enter `handler`, the first time keeping the one it replaces.
M$OSIV proc far
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
M$OSIV endp

; M$OVEC: puts back every vector M$OSIV replaced, the last first. Every
; exit path calls it.
M$OVEC proc far
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
M$OVEC endp

; M$OCHN(handler: extern "interrupt16" fn()): enters `handler` as its
; interrupt would, flags pushed, and comes back.
M$OCHN proc far
    push bp
    mov bp, sp
    pushf
    call dword ptr [bp+6]
    pop bp
    retf
M$OCHN endp

; M$OEXT(code: u8): restores the vectors and ends the program.
M$OEXT proc far
    push bp
    mov bp, sp
    call far ptr M$OVEC
    mov al, [bp+6]
    mov ah, 4ch
    int 21h
M$OEXT endp

end
