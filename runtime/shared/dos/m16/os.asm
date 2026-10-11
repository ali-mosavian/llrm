;; name: LL$OPEN, LL$CREATE, LL$READ, LL$SEEK, LL$WRITE_FILE, LL$CLOSE, LL$EXIT, LL$MORE, LL$BLOCK_RESIZE, LL$VECTOR, LL$SET_VECTOR, LL$CHAIN, LL$RESTORE_VECTORS, LL$CONSOLE_READ_KEY, LL$CONSOLE_KEY_READY, LL$COMMAND_LINE, LL$ENVIRONMENT
;; desc: the OS layer shared by C, Nib and BASIC programs: x86-m16, real-mode DOS
;;
;; args: the interface of runtime/shared/interface.toml, in cdecl16: arguments in word slots from [bp+6],
;;       a far pointer being two of them
;; retn: ax (dx:ax for a long) -> the result, or the DOS error code negated
;;
;; chng: oct/26 restyled, symbols renamed to LL$ [ali]
;; obs.: Each routine is named LL$<OP> for the operation of the interface (the QB runtime's own
;;       entries are B$, and its devices are not here). The file holds one object per group: a
;;       library is built with -DOS_GROUPS=<bit>, so a program links what it calls; assembled with
;;       no definition it holds every group.

.model medium
.386

;; Which groups of the interface this object holds. A library has one object per group, each
;; assembled with -DOS_GROUPS=<its bit>, so a program links only what it calls; assembled with no
;; definition the file holds every group.
G_CORE          equ     1
G_CONSOLE       equ     2
G_PROCESS       equ     4
ifndef OS_GROUPS
OS_GROUPS       equ     7
endif

if OS_GROUPS and G_CORE
public LL$OPEN
public LL$CREATE
public LL$READ
public LL$SEEK
public LL$BLOCK_RESIZE
public LL$WRITE_FILE
public LL$CLOSE
public LL$EXIT
endif
if OS_GROUPS and G_CONSOLE
public LL$CONSOLE_READ_KEY
public LL$CONSOLE_KEY_READY
endif
if OS_GROUPS and G_PROCESS
public LL$COMMAND_LINE
public LL$ENVIRONMENT
if (OS_GROUPS and G_CORE) eq 0
extrn LL$PSP:word
endif
endif
if OS_GROUPS and G_CORE
public LL$MORE
public LL$VECTOR
public LL$SET_VECTOR
public LL$CHAIN
public LL$RESTORE_VECTORS
public LL$TOP
public LL$STACK_LOW
public LL$PSP
public BSS_LAST
public FBSS_LAST
endif

;; What DOS's device information (IOCTL 44h) says of a handle: bit 7 is a character device.
DEVICE_BIT      equ     80h
CTRL_Z          equ     1Ah

if OS_GROUPS and G_CORE
;; Where the uninitialised data ends, near and far, which start.asm zeroes: this object is linked last.
_BSS            segment word public 'BSS'
BSS_LAST        label   byte
_BSS            ends
FBSS_END        segment para public 'FAR_BSS'
FBSS_LAST       label   byte
                db      16 dup (?)
FBSS_END        ends

.data
;; The near heap's end, a DGROUP offset, and the program's PSP, whose
;; memory block the heap grows. Startup sets both.
LL$TOP          dw      0
LL$PSP          dw      0
;; The lowest SP a checked function may reach (-fsanitize=stack): the stack's bottom plus the
;; reserve the panic, DOS and an interrupt use below it. Startup sets it.
LL$STACK_LOW    dw      0

;; Each vector the program replaced, and what it entered before, which
;; LL$RESTORE_VECTORS puts back. A vector replaced past the last entry is not.
SAVED           equ     16
saved_count     dw      0
saved_number    db      SAVED dup (0)
saved_old       dd      SAVED dup (0)
endif


.code
if OS_GROUPS and G_CORE
;;::::::::::::::
;; LL$MORE(bytes: u16) -> *near mut u8: `bytes` more of DGROUP at the heap's end, or
;; 0 when DGROUP's 64 KB or DOS's memory runs out.
LL$MORE         proc    far
                push    bp
                mov     bp, sp
                push    bx
                push    es
                mov     bx, LL$TOP
                add     bx, [bp+6]
                jc      short refused
                cmp     bx, 0fff0h
                ja      short refused
                mov     ax, bx
                add     ax, 15
                shr     ax, 4
                mov     bx, DGROUP
                sub     bx, LL$PSP
                add     bx, ax
                mov     es, LL$PSP
                mov     ah, DOS_RESIZE
                int     DOS_INT
                jc      short refused
                mov     ax, LL$TOP
                mov     bx, [bp+6]
                add     LL$TOP, bx
                jmp     short grown
refused:
                xor     ax, ax
grown:
                pop     es
                pop     bx
                pop     bp
                retf
LL$MORE         endp

;; The DOS file calls. Each returns what DOS does, or when DOS sets carry,
;; its error code negated.

;;::::::::::::::
;; LL$OPEN(name: *far char, mode: u8) -> i16: a handle to an existing file.
LL$OPEN         proc    far
                push    bp
                mov     bp, sp
                push    ds
                lds     dx, [bp+6]
                mov     al, [bp+10]
                mov     ah, DOS_OPEN
                jmp     short called
LL$OPEN         endp

;;::::::::::::::
;; LL$CREATE(name: *far char) -> i16: a handle to a new, empty file.
LL$CREATE       proc    far
                push    bp
                mov     bp, sp
                push    ds
                lds     dx, [bp+6]
                xor     cx, cx
                mov     ah, DOS_CREATE
                jmp     short called
LL$CREATE       endp

;;::::::::::::::
;; LL$READ(handle: i16, data: *far mut u8, count: u16) -> i32: bytes read, or the error code negated: a count
;; to 65535 and a sign take 17 bits, so the result is DX:AX.
LL$READ         proc    far
                mov     ah, DOS_READ
                jmp     short transfer
LL$READ         endp

;;::::::::::::::
;; LL$SEEK(handle: i16, position: i32, origin: u8) -> i32: the absolute position, or the error code negated.
LL$SEEK         proc    far
                push    bp
                mov     bp, sp
                push    bx
                mov     bx, [bp+6]
                mov     dx, [bp+8]
                mov     cx, [bp+10]
                mov     al, [bp+12]
                mov     ah, DOS_SEEK
                int     DOS_INT
                pop     bx
                jc      short seek_failed
                pop     bp
                retf
seek_failed:
                neg     ax
                cwd
                pop     bp
                retf
LL$SEEK         endp

;;::::::::::::::
;; LL$BLOCK_RESIZE(segment: u16, paragraphs: u16) -> u16: the paragraphs the block has once DOS 4Ah has
;; resized it to `paragraphs`, or to the most there is when that is more.
LL$BLOCK_RESIZE proc    far
                push    bp
                mov     bp, sp
                push    bx
                push    es
                mov     es, [bp+6]
                mov     bx, [bp+8]
                mov     ah, DOS_RESIZE
                int     DOS_INT
                jnc     short resized
                mov     ah, DOS_RESIZE          ;; BX is the most there is: take it
                int     DOS_INT
resized:
                mov     ax, bx
                pop     es
                pop     bx
                pop     bp
                retf
LL$BLOCK_RESIZE endp

;;::::::::::::::
;; LL$WRITE_FILE(handle: i16, data: *far u8, count: u16) -> i32: bytes written, as LL$READ.
LL$WRITE_FILE   proc    far
                mov     ah, DOS_WRITE
transfer::
                push    bp
                mov     bp, sp
                push    ds
                push    bx
                mov     bx, [bp+6]
                lds     dx, [bp+8]
                mov     cx, [bp+12]
                int     DOS_INT
                pop     bx
                jc      short failed
                xor     dx, dx
                jmp     short transferred
failed:
                neg     ax
                cwd
transferred:
                pop     ds
                pop     bp
                retf
LL$WRITE_FILE   endp

;;::::::::::::::
;; LL$CLOSE(handle: handle) -> isize
LL$CLOSE        proc    far
                push    bp
                mov     bp, sp
                push    ds
                push    bx
                mov     bx, [bp+6]
                mov     ah, DOS_CLOSE
                int     DOS_INT
                pop     bx
                jc      short checked
                xor     ax, ax                  ;; DOS leaves AX undefined on success
                jmp     short checked
called::
                int     DOS_INT
checked::
                jnc     short done
                neg     ax
done:
                pop     ds
                pop     bp
                retf
LL$CLOSE        endp

endif

if OS_GROUPS and G_CONSOLE
;; The console's keyboard: standard input, so what DOS redirects it follows.

;;::::::::::::::
;; LL$CONSOLE_READ_KEY() -> u8: the keyboard's character, no echo (08h). From redirected
;; input it is the next byte read from the handle, or Ctrl-Z once there are none.
LL$CONSOLE_READ_KEY proc    far
                push    bx
                push    cx
                push    dx
                mov     ax, DOS_IOCTL * 256
                mov     bx, DOS_STDIN
                int     DOS_INT
                jc      short from_handle
                test    dl, DEVICE_BIT
                jnz     short from_device
from_handle:
                push    ax                      ;; the byte lands here
                mov     ah, DOS_READ
                mov     bx, DOS_STDIN
                mov     cx, 1
                mov     dx, sp
                int     DOS_INT
                pop     dx
                jc      short end_of_input
                cmp     ax, 1
                jne     short end_of_input
                mov     al, dl
                jmp     short key_done
end_of_input:
                mov     al, CTRL_Z
                jmp     short key_done
from_device:
                mov     ah, DOS_READ_KEY
                int     DOS_INT
key_done:
                xor     ah, ah
                pop     dx
                pop     cx
                pop     bx
                retf
LL$CONSOLE_READ_KEY endp

;;::::::::::::::
;; LL$CONSOLE_KEY_READY() -> bool: DOS's input status (0Bh), 0FFh or 0, as 1 or 0.
LL$CONSOLE_KEY_READY proc    far
                mov     ah, DOS_KEY_READY
                int     DOS_INT
                and     ax, 1
                retf
LL$CONSOLE_KEY_READY endp
endif

if OS_GROUPS and G_CORE
;; Interrupt vectors, for handlers the program installs. A handler is
;; entered with interrupts off and leaves by iret.

;;::::::::::::::
;; LL$VECTOR(number: u8) -> extern "interrupt16" fn(): what interrupt `number`
;; enters now.
LL$VECTOR       proc    far
                push    bp
                mov     bp, sp
                push    bx
                push    es
                mov     al, [bp+6]
                mov     ah, DOS_GET_VECTOR
                int     DOS_INT
                mov     ax, bx
                mov     dx, es
                pop     es
                pop     bx
                pop     bp
                retf
LL$VECTOR       endp

;;::::::::::::::
;; LL$SET_VECTOR(number: u8, handler: extern "interrupt16" fn()): makes interrupt
;; `number` enter `handler`, the first time keeping the one it replaces.
LL$SET_VECTOR   proc    far
                push    bp
                mov     bp, sp
                push    bx
                push    si
                push    es
                mov     al, [bp+6]
                xor     si, si
seek:
                cmp     si, saved_count
                je      short keep
                cmp     saved_number[si], al
                je      short install
                inc     si
                jmp     short seek
keep:
                cmp     si, SAVED
                je      short install
                mov     saved_number[si], al
                mov     ah, DOS_GET_VECTOR
                int     DOS_INT
                shl     si, 2
                mov     word ptr saved_old[si], bx
                mov     word ptr saved_old[si+2], es
                inc     saved_count
install:
                push    ds
                lds     dx, [bp+8]
                mov     al, [bp+6]
                mov     ah, DOS_SET_VECTOR
                int     DOS_INT
                pop     ds
                pop     es
                pop     si
                pop     bx
                pop     bp
                retf
LL$SET_VECTOR   endp

;;::::::::::::::
;; LL$RESTORE_VECTORS: puts back every vector LL$SET_VECTOR replaced, the last first. Every
;; exit path calls it.
LL$RESTORE_VECTORS proc    far
                push    ds
                push    si
                mov     ax, DGROUP
                mov     ds, ax
                mov     si, saved_count
                mov     saved_count, 0
restore:
                dec     si
                js      short restored
                mov     al, saved_number[si]
                push    si
                shl     si, 2
                push    ds
                lds     dx, saved_old[si]
                mov     ah, DOS_SET_VECTOR
                int     DOS_INT
                pop     ds
                pop     si
                jmp     short restore
restored:
                pop     si
                pop     ds
                retf
LL$RESTORE_VECTORS endp

;;::::::::::::::
;; LL$CHAIN(handler: extern "interrupt16" fn()): enters `handler` as its
;; interrupt would, flags pushed, and comes back.
LL$CHAIN        proc    far
                push    bp
                mov     bp, sp
                pushf
                call    dword ptr [bp+6]
                pop     bp
                retf
LL$CHAIN        endp

;;::::::::::::::
;; LL$EXIT(code: u8): restores the vectors and ends the program.
LL$EXIT         proc    far
                push    bp
                mov     bp, sp
                call    far ptr LL$RESTORE_VECTORS
                mov     al, [bp+6]
                mov     ah, DOS_EXIT
                int     DOS_INT
LL$EXIT         endp
endif

if OS_GROUPS and G_PROCESS
;; The program's PSP holds its command tail (length at 80h, text from 81h) and the environment's
;; paragraph (at 2Ch); the environment is NUL-ended strings ended by one more NUL.
PSP_ENVIRONMENT equ     2Ch
PSP_TAIL        equ     80h

;;::::::::::::::
;; LL$COMMAND_LINE(data: *far mut u8, max: u16) -> i16: the command tail without its leading blanks,
;; at most `max` bytes of it, and the length copied.
LL$COMMAND_LINE proc    far
                push    bp
                mov     bp, sp
                push    si
                push    di
                push    ds
                push    es
                les     di, [bp+6]
                mov     cx, [bp+10]
                mov     ds, LL$PSP
                mov     si, PSP_TAIL
                lodsb
                xor     ah, ah
                mov     bx, ax                  ;; the tail's length
skip_blank:
                test    bx, bx
                jz      short tail_copy
                cmp     byte ptr [si], ' '
                jne     short tail_copy
                inc     si
                dec     bx
                jmp     short skip_blank
tail_copy:
                cmp     bx, cx
                jbe     short tail_fits
                mov     bx, cx
tail_fits:
                mov     cx, bx
                mov     ax, bx
                rep     movsb
                pop     es
                pop     ds
                pop     di
                pop     si
                pop     bp
                retf
LL$COMMAND_LINE endp

;;::::::::::::::
;; LL$ENVIRONMENT(index: u16, data: *far mut u8, max: u16) -> i16: the `index`th string of the
;; environment (from 0), at most `max` bytes of it, and its length; -1 past the last.
LL$ENVIRONMENT  proc    far
                push    bp
                mov     bp, sp
                push    si
                push    di
                push    ds
                push    es
                mov     ds, LL$PSP
                mov     ax, ds:[PSP_ENVIRONMENT]
                mov     ds, ax
                xor     si, si
                mov     cx, [bp+6]
next_string:
                cmp     byte ptr [si], 0
                je      short no_string         ;; the closing NUL: past the last
                jcxz    short found
                dec     cx
skip_string:
                lodsb
                test    al, al
                jnz     short skip_string
                jmp     short next_string
found:
                mov     bx, si
measure:
                lodsb
                test    al, al
                jnz     short measure
                dec     si
                sub     si, bx                  ;; its length
                mov     ax, si
                mov     si, bx
                les     di, [bp+8]
                mov     cx, [bp+12]
                cmp     ax, cx
                jbe     short string_fits
                mov     ax, cx
string_fits:
                mov     cx, ax
                rep     movsb
                jmp     short environment_done
no_string:
                mov     ax, -1
environment_done:
                pop     es
                pop     ds
                pop     di
                pop     si
                pop     bp
                retf
LL$ENVIRONMENT  endp
endif

end
