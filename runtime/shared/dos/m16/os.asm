.model medium
.386

public _llrm_os_open
public _llrm_os_create
public _llrm_os_read
public _llrm_os_seek
public _llrm_os_block_resize
public _llrm_os_write_file
public _llrm_os_close
public _llrm_os_exit
public _llrm_os_console_read_key
public _llrm_os_console_key_ready
public _llrm_os_more
public _llrm_os_vector
public _llrm_os_set_vector
public _llrm_os_chain
public _llrm_os_restore_vectors
public _llrm_os_top
public _llrm_os_stack_low
public _llrm_os_psp
public BSS_LAST
public FBSS_LAST

; Where the uninitialised data ends, near and far, which start.asm zeroes: this object is linked last.
_BSS segment word public 'BSS'
BSS_LAST label byte
_BSS ends
FBSS_END segment para public 'FAR_BSS'
FBSS_LAST label byte
    db 16 dup (?)
FBSS_END ends

.data
; The near heap's end, a DGROUP offset, and the program's PSP, whose
; memory block the heap grows. Startup sets both.
_llrm_os_top dw 0
_llrm_os_psp dw 0
; The lowest SP a checked function may reach (-fsanitize=stack): the stack's bottom plus the
; reserve the panic, DOS and an interrupt use below it. Startup sets it.
_llrm_os_stack_low dw 0

; Each vector the program replaced, and what it entered before, which
; _llrm_os_restore_vectors puts back. A vector replaced past the last entry is not.
SAVED equ 16
saved_count dw 0
saved_number db SAVED dup (0)
saved_old dd SAVED dup (0)

.code
; _llrm_os_more(bytes: u16) -> *near mut u8: `bytes` more of DGROUP at the heap's end, or
; 0 when DGROUP's 64 KB or DOS's memory runs out.
_llrm_os_more proc far
    push bp
    mov bp, sp
    push bx
    push es
    mov bx, _llrm_os_top
    add bx, [bp+6]
    jc short refused
    cmp bx, 0fff0h
    ja short refused
    mov ax, bx
    add ax, 15
    shr ax, 4
    mov bx, DGROUP
    sub bx, _llrm_os_psp
    add bx, ax
    mov es, _llrm_os_psp
    mov ah, DOS_RESIZE
    int DOS_INT
    jc short refused
    mov ax, _llrm_os_top
    mov bx, [bp+6]
    add _llrm_os_top, bx
    jmp short grown
refused:
    xor ax, ax
grown:
    pop es
    pop bx
    pop bp
    retf
_llrm_os_more endp

; The DOS file calls. Each returns what DOS does, or when DOS sets carry,
; its error code negated.

; _llrm_os_open(name: *far char, mode: u8) -> i16: a handle to an existing file.
_llrm_os_open proc far
    push bp
    mov bp, sp
    push ds
    lds dx, [bp+6]
    mov al, [bp+10]
    mov ah, DOS_OPEN
    jmp short called
_llrm_os_open endp

; _llrm_os_create(name: *far char) -> i16: a handle to a new, empty file.
_llrm_os_create proc far
    push bp
    mov bp, sp
    push ds
    lds dx, [bp+6]
    xor cx, cx
    mov ah, DOS_CREATE
    jmp short called
_llrm_os_create endp

; _llrm_os_read(handle: i16, data: *far mut u8, count: u16) -> i32: bytes read, or the error code negated: a count
; to 65535 and a sign take 17 bits, so the result is DX:AX.
_llrm_os_read proc far
    mov ah, DOS_READ
    jmp short transfer
_llrm_os_read endp

; _llrm_os_seek(handle: i16, position: i32, origin: u8) -> i32: the absolute position, or the error code negated.
_llrm_os_seek proc far
    push bp
    mov bp, sp
    push bx
    mov bx, [bp+6]
    mov dx, [bp+8]
    mov cx, [bp+10]
    mov al, [bp+12]
    mov ah, DOS_SEEK
    int DOS_INT
    pop bx
    jc short seek_failed
    pop bp
    retf
seek_failed:
    neg ax
    cwd
    pop bp
    retf
_llrm_os_seek endp

; _llrm_os_block_resize(segment: u16, paragraphs: u16) -> u16: the paragraphs the block has once DOS 4Ah has
; resized it to `paragraphs`, or to the most there is when that is more.
_llrm_os_block_resize proc far
    push bp
    mov bp, sp
    push bx
    push es
    mov es, [bp+6]
    mov bx, [bp+8]
    mov ah, DOS_RESIZE
    int DOS_INT
    jnc short resized
    mov ah, DOS_RESIZE             ; BX is the most there is: take it
    int DOS_INT
resized:
    mov ax, bx
    pop es
    pop bx
    pop bp
    retf
_llrm_os_block_resize endp

; _llrm_os_write_file(handle: i16, data: *far u8, count: u16) -> i32: bytes written, as _llrm_os_read.
_llrm_os_write_file proc far
    mov ah, DOS_WRITE
transfer::
    push bp
    mov bp, sp
    push ds
    push bx
    mov bx, [bp+6]
    lds dx, [bp+8]
    mov cx, [bp+12]
    int DOS_INT
    pop bx
    jc short failed
    xor dx, dx
    jmp short transferred
failed:
    neg ax
    cwd
transferred:
    pop ds
    pop bp
    retf
_llrm_os_write_file endp

; _llrm_os_close(handle: handle) -> isize
_llrm_os_close proc far
    push bp
    mov bp, sp
    push ds
    push bx
    mov bx, [bp+6]
    mov ah, DOS_CLOSE
    int DOS_INT
    pop bx
    jc short checked
    xor ax, ax                     ; DOS leaves AX undefined on success
    jmp short checked
called::
    int DOS_INT
checked::
    jnc short done
    neg ax
done:
    pop ds
    pop bp
    retf
_llrm_os_close endp

; The console's keyboard: standard input, so what DOS redirects it follows.

; _llrm_os_console_read_key() -> u8: DOS's character input without echo (08h).
_llrm_os_console_read_key proc far
    mov ah, DOS_READ_KEY
    int DOS_INT
    xor ah, ah
    retf
_llrm_os_console_read_key endp

; _llrm_os_console_key_ready() -> bool: DOS's input status (0Bh), 0FFh or 0, as 1 or 0.
_llrm_os_console_key_ready proc far
    mov ah, DOS_KEY_READY
    int DOS_INT
    and ax, 1
    retf
_llrm_os_console_key_ready endp

; Interrupt vectors, for handlers the program installs. A handler is
; entered with interrupts off and leaves by iret.

; _llrm_os_vector(number: u8) -> extern "interrupt16" fn(): what interrupt `number`
; enters now.
_llrm_os_vector proc far
    push bp
    mov bp, sp
    push bx
    push es
    mov al, [bp+6]
    mov ah, DOS_GET_VECTOR
    int DOS_INT
    mov ax, bx
    mov dx, es
    pop es
    pop bx
    pop bp
    retf
_llrm_os_vector endp

; _llrm_os_set_vector(number: u8, handler: extern "interrupt16" fn()): makes interrupt
; `number` enter `handler`, the first time keeping the one it replaces.
_llrm_os_set_vector proc far
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
    mov ah, DOS_GET_VECTOR
    int DOS_INT
    shl si, 2
    mov word ptr saved_old[si], bx
    mov word ptr saved_old[si+2], es
    inc saved_count
install:
    push ds
    lds dx, [bp+8]
    mov al, [bp+6]
    mov ah, DOS_SET_VECTOR
    int DOS_INT
    pop ds
    pop es
    pop si
    pop bx
    pop bp
    retf
_llrm_os_set_vector endp

; _llrm_os_restore_vectors: puts back every vector _llrm_os_set_vector replaced, the last first. Every
; exit path calls it.
_llrm_os_restore_vectors proc far
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
    mov ah, DOS_SET_VECTOR
    int DOS_INT
    pop ds
    pop si
    jmp short restore
restored:
    pop si
    pop ds
    retf
_llrm_os_restore_vectors endp

; _llrm_os_chain(handler: extern "interrupt16" fn()): enters `handler` as its
; interrupt would, flags pushed, and comes back.
_llrm_os_chain proc far
    push bp
    mov bp, sp
    pushf
    call dword ptr [bp+6]
    pop bp
    retf
_llrm_os_chain endp

; _llrm_os_exit(code: u8): restores the vectors and ends the program.
_llrm_os_exit proc far
    push bp
    mov bp, sp
    call far ptr _llrm_os_restore_vectors
    mov al, [bp+6]
    mov ah, DOS_EXIT
    int DOS_INT
_llrm_os_exit endp

end
