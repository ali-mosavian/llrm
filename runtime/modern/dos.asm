.model medium
.386

extrn _rt_restore_vectors:far

public _rt_write
public _rt_exit
public _dos_create
public _dos_write
public _dos_close

.code
_rt_write proc far
    push bp
    mov bp, sp
    push bx
    mov dx, [bp+6]
    mov cx, [bp+8]
    mov bx, 1
    mov ah, 40h
    int 21h
    pop bx
    pop bp
    retf
_rt_write endp

; cdecl16 file routines for `extern` declarations. Each returns -1 when DOS
; sets carry.

; i16 dos_create(char far *name): a handle to a new, empty file.
_dos_create proc far
    push bp
    mov bp, sp
    push ds
    lds dx, [bp+6]
    xor cx, cx
    mov ah, 3ch
    int 21h
    jnc created
    mov ax, -1
created:
    pop ds
    pop bp
    retf
_dos_create endp

; i16 dos_write(i16 handle, char far *data, u16 count): bytes written.
_dos_write proc far
    push bp
    mov bp, sp
    push bx
    push ds
    mov bx, [bp+6]
    lds dx, [bp+8]
    mov cx, [bp+12]
    mov ah, 40h
    int 21h
    jnc written
    mov ax, -1
written:
    pop ds
    pop bx
    pop bp
    retf
_dos_write endp

; void dos_close(i16 handle)
_dos_close proc far
    push bp
    mov bp, sp
    push bx
    mov bx, [bp+6]
    mov ah, 3eh
    int 21h
    pop bx
    pop bp
    retf
_dos_close endp

_rt_exit proc far
    push bp
    mov bp, sp
    call far ptr _rt_restore_vectors
    mov al, [bp+6]
    mov ah, 4ch
    int 21h
_rt_exit endp

end
