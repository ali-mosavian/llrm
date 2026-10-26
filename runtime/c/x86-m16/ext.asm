; The C run tests' externals, on the OS layer: the report stream (one signed decimal per line), the
; benchmark's input and the opaque calls of spec.OPAQUE.
ifdef NEAR_CODE
.model small
LINK equ <near>
ARG equ 4
else
.model medium
LINK equ <far>
ARG equ 6
endif
.386

extrn LL$OPEN:far
extrn LL$READ:far
extrn LL$WRITE_FILE:far
extrn LL$EXIT:far

.data
ticks   dw 0
digits  db 13 dup (?)
inhandle dw 0FFFFh
inname  db 'DICKENS', 0

stkmsg  db 'Stack Overflow!', 13, 10

.code
; Open Watcom's stack overflow (clib stk086.asm `__STKOVERFLOW`): the message and exit status 1,
; which a checked function enters (-fsanitize=stack). On stdout, which a test captures.
public __STKOVERFLOW
__STKOVERFLOW proc LINK
    mov ax, @data
    mov ds, ax
    push 17
    push ds
    push offset stkmsg
    push DOS_STDOUT
    call far ptr LL$WRITE_FILE
    push 1
    call far ptr LL$EXIT
__STKOVERFLOW endp

; void report(long v)
public _report
_report proc LINK
    push bp
    mov bp, sp
    push si
    push di
    mov eax, dword ptr [bp+ARG]
    mov di, offset digits + 13
    mov byte ptr [di-1], 10
    mov byte ptr [di-2], 13
    sub di, 2
    xor cx, cx
    test eax, eax
    jns positive
    neg eax
    inc cx
positive:
    mov ebx, 10
more:
    xor edx, edx
    div ebx
    add dl, '0'
    dec di
    mov [di], dl
    test eax, eax
    jnz more
    jcxz unsigned
    dec di
    mov byte ptr [di], '-'
unsigned:
    mov cx, offset digits + 13
    sub cx, di
    push cx
    push ds
    push di
    push DOS_STDOUT
    call far ptr LL$WRITE_FILE
    add sp, 8
    pop di
    pop si
    pop bp
    ret
_report endp

; int input_read(char *buffer, int count): up to count bytes of DICKENS (opened on the first call, in the
; current directory), 0 at its end or on an error. The bench/grep program reads its input through this.
public _input_read
_input_read proc LINK
    push bp
    mov bp, sp
    cmp inhandle, 0FFFFh
    jne opened
    push 0
    push ds
    push offset inname
    call far ptr LL$OPEN
    add sp, 6
    test ax, ax
    js failed
    mov inhandle, ax
opened:
    push word ptr [bp+ARG+2]
    push ds
    push word ptr [bp+ARG]
    push inhandle
    call far ptr LL$READ
    add sp, 8
    test dx, dx
    jns done
failed:
    xor ax, ax
done:
    pop bp
    ret
_input_read endp

; void *memmove(void *dst, const void *src, unsigned n), near pointers that may overlap, in regparm3 (the default convention):
; dst in AX, src in DX, n in CX; dst comes back in AX. A copy with dst above src inside it runs backwards under the
; direction flag, cleared again before the return: the n mod 4 end bytes first, then dwords. SI and DI are kept.
public _memmove@3
_memmove@3 proc LINK
    push si
    push di
    push ax
    mov ax, ds
    mov es, ax
    pop ax
    mov di, ax
    mov si, dx
    cmp di, si
    jbe forward
    mov dx, si
    add dx, cx
    cmp di, dx
    jae forward
    mov si, dx
    dec si
    add di, cx
    dec di
    mov dx, cx
    and cx, 3
    std
    rep movsb
    sub si, 3
    sub di, 3
    mov cx, dx
    shr cx, 2
    rep movsd
    cld
    jmp finished
forward:
    mov dx, cx
    shr cx, 2
    rep movsd
    mov cx, dx
    and cx, 3
    rep movsb
finished:
    pop di
    pop si
    ret
_memmove@3 endp

; int keep(int x)
public _keep
_keep proc LINK
    push bp
    mov bp, sp
    mov ax, [bp+ARG]
    pop bp
    ret
_keep endp

; long keep32(long x)
public _keep32
_keep32 proc LINK
    push bp
    mov bp, sp
    mov ax, [bp+ARG]
    mov dx, [bp+ARG+2]
    pop bp
    ret
_keep32 endp

public _touch
_touch proc LINK
    ret
_touch endp

public _tick
_tick proc LINK
    inc word ptr ticks
    ret
_tick endp

; unsigned tick_count(void): the count since the last call
public _tick_count
_tick_count proc LINK
    xor ax, ax
    xchg ax, word ptr ticks
    ret
_tick_count endp

; void lcopy(void far *d, void far *s, unsigned n)
public _lcopy
_lcopy proc LINK
    push bp
    mov bp, sp
    push si
    push di
    push ds
    push es
    les di, [bp+ARG]
    lds si, [bp+ARG+4]
    mov cx, [bp+ARG+8]
    cld
    rep movsb
    pop es
    pop ds
    pop di
    pop si
    pop bp
    ret
_lcopy endp

ifndef NEAR_CODE
; The same routines for a program in Open Watcom's register convention (-mabi=watcom): a long in DX:AX, an int in AX, a near
; pointer in AX; the callee keeps every register but AX and its arguments'. They call the cdecl ones above.
public report_
report_ proc far
    push bx
    push cx
    push dx
    push ax
    call far ptr _report
    add sp, 4
    pop cx
    pop bx
    retf
report_ endp

public input_read_
input_read_ proc far
    push bx
    push cx
    push dx
    push ax
    call far ptr _input_read
    add sp, 4
    pop cx
    pop bx
    retf
input_read_ endp

public keep_
keep_ proc far
    retf
keep_ endp

public keep32_
keep32_ proc far
    retf
keep32_ endp

; And for a program in regparm3 (-mabi=regparm3, the default): a long in EAX, an int or a near pointer in AX, then DX, CX.
public _report@3
_report@3 proc far
    push eax
    call far ptr _report
    add sp, 4
    retf
_report@3 endp

public _input_read@3
_input_read@3 proc far
    push dx
    push ax
    call far ptr _input_read
    add sp, 4
    retf
_input_read@3 endp

public _keep@3
_keep@3 proc far
    retf
_keep@3 endp

public _keep32@3
_keep32@3 proc far
    retf
_keep32@3 endp

public _touch@3
_touch@3 proc far
    retf
_touch@3 endp

public _tick@3
_tick@3 proc far
    inc word ptr ticks
    retf
_tick@3 endp

public _tick_count@3
_tick_count@3 proc far
    xor ax, ax
    xchg ax, word ptr ticks
    retf
_tick_count@3 endp

; void lcopy(void far *d, void far *s, unsigned n): n in AX, the pointers on the stack
public _lcopy@3
_lcopy@3 proc far
    push bp
    mov bp, sp
    push ax
    push word ptr [bp+12]
    push word ptr [bp+10]
    push word ptr [bp+8]
    push word ptr [bp+6]
    call far ptr _lcopy
    add sp, 10
    pop bp
    retf
_lcopy@3 endp

endif

end
