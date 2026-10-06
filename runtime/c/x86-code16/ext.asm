; The C run tests' externals, on the OS layer: the report stream (one signed decimal per line), the
; benchmark's input and the opaque calls of spec.OPAQUE.
.model medium
.386

extrn _llrm_os_open:far
extrn _llrm_os_read:far
extrn _llrm_os_write_file:far
extrn _llrm_os_exit:far

.data
ticks   dw 0
digits  db 12 dup (?)
inhandle dw 0FFFFh
inname  db 'DICKENS', 0

stkmsg  db 'Stack Overflow!', 13, 10

.code
; Open Watcom's stack overflow (clib stk086.asm `__STKOVERFLOW`): the message and exit status 1,
; which a checked function enters (-fsanitize=stack). On stdout, which a test captures.
public __STKOVERFLOW
__STKOVERFLOW proc far
    mov ax, @data
    mov ds, ax
    push 17
    push ds
    push offset stkmsg
    push 1
    call far ptr _llrm_os_write_file
    push 1
    call far ptr _llrm_os_exit
__STKOVERFLOW endp

; void report(long v)
public _report
_report proc far
    push bp
    mov bp, sp
    push si
    push di
    mov eax, dword ptr [bp+6]
    mov di, offset digits + 12
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
    mov cx, offset digits + 12
    sub cx, di
    push cx
    push ds
    push di
    push 1
    call far ptr _llrm_os_write_file
    add sp, 8
    pop di
    pop si
    pop bp
    ret
_report endp

; int input_read(char *buffer, int count): up to count bytes of DICKENS (opened on the first call, in the
; current directory), 0 at its end or on an error. The bench/grep program reads its input through this.
public _input_read
_input_read proc far
    push bp
    mov bp, sp
    cmp inhandle, 0FFFFh
    jne opened
    push 0
    push ds
    push offset inname
    call far ptr _llrm_os_open
    add sp, 6
    test ax, ax
    js failed
    mov inhandle, ax
opened:
    push word ptr [bp+8]
    push ds
    push word ptr [bp+6]
    push inhandle
    call far ptr _llrm_os_read
    add sp, 8
    test dx, dx
    jns done
failed:
    xor ax, ax
done:
    pop bp
    ret
_input_read endp

; int keep(int x)
public _keep
_keep proc far
    push bp
    mov bp, sp
    mov ax, [bp+6]
    pop bp
    ret
_keep endp

; long keep32(long x)
public _keep32
_keep32 proc far
    push bp
    mov bp, sp
    mov ax, [bp+6]
    mov dx, [bp+8]
    pop bp
    ret
_keep32 endp

public _touch
_touch proc far
    ret
_touch endp

public _tick
_tick proc far
    inc word ptr ticks
    ret
_tick endp

; unsigned tick_count(void): the count since the last call
public _tick_count
_tick_count proc far
    xor ax, ax
    xchg ax, word ptr ticks
    ret
_tick_count endp

; void lcopy(void far *d, void far *s, unsigned n)
public _lcopy
_lcopy proc far
    push bp
    mov bp, sp
    push si
    push di
    push ds
    push es
    les di, [bp+6]
    lds si, [bp+10]
    mov cx, [bp+14]
    cld
    rep movsb
    pop es
    pop ds
    pop di
    pop si
    pop bp
    ret
_lcopy endp

end
