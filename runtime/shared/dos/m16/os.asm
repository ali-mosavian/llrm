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
public _llrm_os_clock_hundredths
public _llrm_os_speaker_tone
public _llrm_os_screen_is_console
public _llrm_os_screen_size
public _llrm_os_screen_cursor
public _llrm_os_screen_move
public _llrm_os_screen_put
public _llrm_os_screen_write
public _llrm_os_screen_get
public _llrm_os_screen_scroll
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

; What DOS's device information (IOCTL 44h) says of a handle: bit 7 is a character device.
DEVICE_BIT equ 80h
CTRL_Z equ 1Ah

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

; _llrm_os_console_read_key() -> u8: the keyboard's character, no echo (08h). From redirected
; input it is the next byte read from the handle, or Ctrl-Z once there are none.
_llrm_os_console_read_key proc far
    push bx
    push cx
    push dx
    mov ax, DOS_IOCTL * 256
    mov bx, DOS_STDIN
    int DOS_INT
    jc short from_handle
    test dl, DEVICE_BIT
    jnz short from_device
from_handle:
    push ax                        ; the byte lands here
    mov ah, DOS_READ
    mov bx, DOS_STDIN
    mov cx, 1
    mov dx, sp
    int DOS_INT
    pop dx
    jc short end_of_input
    cmp ax, 1
    jne short end_of_input
    mov al, dl
    jmp short key_done
end_of_input:
    mov al, CTRL_Z
    jmp short key_done
from_device:
    mov ah, DOS_READ_KEY
    int DOS_INT
key_done:
    xor ah, ah
    pop dx
    pop cx
    pop bx
    retf
_llrm_os_console_read_key endp

; _llrm_os_console_key_ready() -> bool: DOS's input status (0Bh), 0FFh or 0, as 1 or 0.
_llrm_os_console_key_ready proc far
    mov ah, DOS_KEY_READY
    int DOS_INT
    and ax, 1
    retf
_llrm_os_console_key_ready endp

; The speaker: timer channel 2 square wave, gated by port 61h.

TIMER_COMMAND equ 43h
TIMER_CHANNEL_2 equ 42h
SQUARE_WAVE_2 equ 0B6h              ; channel 2, low then high byte, mode 3
SPEAKER_PORT equ 61h
SPEAKER_ON equ 03h                  ; gate and speaker bits

; _llrm_os_speaker_tone(hertz: usize): the tone, or silence for 0.
_llrm_os_speaker_tone proc far
    push bp
    mov bp, sp
    mov cx, [bp+6]
    in al, SPEAKER_PORT
    jcxz short silent
    push ax
    mov dx, DOS_TIMER_CLOCK / 10000h
    mov ax, DOS_TIMER_CLOCK mod 10000h
    div cx
    mov cx, ax
    mov al, SQUARE_WAVE_2
    out TIMER_COMMAND, al
    mov al, cl
    out TIMER_CHANNEL_2, al
    mov al, ch
    out TIMER_CHANNEL_2, al
    pop ax
    or al, SPEAKER_ON
    out SPEAKER_PORT, al
    pop bp
    retf
silent:
    and al, not SPEAKER_ON
    out SPEAKER_PORT, al
    pop bp
    retf
_llrm_os_speaker_tone endp

; The time of day.

SECONDS_PER_MINUTE equ 60
MINUTES_PER_HOUR equ 60
HUNDREDTHS equ 100

; _llrm_os_clock_hundredths() -> i32: DOS's clock (2Ch) as hundredths of a second since midnight.
_llrm_os_clock_hundredths proc far
    push cx
    mov ah, DOS_GET_TIME
    int DOS_INT
    movzx eax, ch
    imul eax, eax, MINUTES_PER_HOUR
    movzx ecx, cl
    add eax, ecx
    imul eax, eax, SECONDS_PER_MINUTE
    movzx ecx, dh
    add eax, ecx
    imul eax, eax, HUNDREDTHS
    movzx ecx, dl
    add eax, ecx
    mov edx, eax
    shr edx, 16
    pop cx
    retf
_llrm_os_clock_hundredths endp

; The screen: the BIOS's text mode. A cell is read and written in video memory, which is how the
; cursor stays where it is; the BIOS data area (0040h) says the mode, columns and rows.

BIOS_DATA equ 0040h
BIOS_COLUMNS equ 004Ah             ; word
BIOS_MODE equ 0049h                ; byte
BIOS_ROWS equ 0084h                ; byte, rows - 1; 0 where the BIOS predates it
MONO_MODE equ 7
MONO_SEGMENT equ 0B000h
COLOR_SEGMENT equ 0B800h
DEFAULT_ROWS equ 25
CONSOLE_OUT_BIT equ 02h

; _llrm_os_screen_is_console() -> bool: whether DOS says standard output is the console device.
_llrm_os_screen_is_console proc far
    push bx
    mov ax, DOS_IOCTL * 256
    mov bx, DOS_STDOUT
    int DOS_INT
    pop bx
    xor ax, ax
    jc short not_console
    and dl, DEVICE_BIT or CONSOLE_OUT_BIT
    cmp dl, DEVICE_BIT or CONSOLE_OUT_BIT
    jne short not_console
    inc ax
not_console:
    retf
_llrm_os_screen_is_console endp

; _llrm_os_screen_size() -> u16: rows in the high byte, columns in the low.
_llrm_os_screen_size proc far
    push ds
    mov ax, BIOS_DATA
    mov ds, ax
    mov al, ds:[BIOS_COLUMNS]
    mov ah, ds:[BIOS_ROWS]
    pop ds
    test ah, ah
    jnz short have_rows
    mov ah, DEFAULT_ROWS - 1
have_rows:
    inc ah
    retf
_llrm_os_screen_size endp

; _llrm_os_screen_cursor() -> u16: the hardware cursor, row in the high byte.
_llrm_os_screen_cursor proc far
    push bx
    push cx
    push dx
    xor bh, bh
    mov ah, DOS_VIDEO_GET_CURSOR
    int DOS_VIDEO_INT
    mov ax, dx
    pop dx
    pop cx
    pop bx
    retf
_llrm_os_screen_cursor endp

; _llrm_os_screen_move(row: u8, column: u8): the hardware cursor.
_llrm_os_screen_move proc far
    push bp
    mov bp, sp
    push bx
    mov dh, [bp+6]
    mov dl, [bp+8]
    xor bh, bh
    mov ah, DOS_VIDEO_SET_CURSOR
    int DOS_VIDEO_INT
    pop bx
    pop bp
    retf
_llrm_os_screen_move endp

; es:di -> the cell at row dh, column dl of the screen; the mode's own video segment.
cell_address proc near
    push ax
    push ds
    mov ax, BIOS_DATA
    mov ds, ax
    mov ax, COLOR_SEGMENT
    cmp byte ptr ds:[BIOS_MODE], MONO_MODE
    jne short have_segment
    mov ax, MONO_SEGMENT
have_segment:
    mov es, ax
    mov al, dh
    mul byte ptr ds:[BIOS_COLUMNS]
    xor dh, dh
    add ax, dx
    add ax, ax
    mov di, ax
    pop ds
    pop ax
    ret
cell_address endp

; _llrm_os_screen_put(row: u8, column: u8, character: u8, attribute: u8)
_llrm_os_screen_put proc far
    push bp
    mov bp, sp
    push di
    push es
    mov dh, [bp+6]
    mov dl, [bp+8]
    call cell_address
    mov al, [bp+10]
    mov ah, [bp+12]
    stosw
    pop es
    pop di
    pop bp
    retf
_llrm_os_screen_put endp

; _llrm_os_screen_write(row: u8, column: u8, text: bytes, count: usize, attribute: u8)
_llrm_os_screen_write proc far
    push bp
    mov bp, sp
    push si
    push di
    push ds
    push es
    mov dh, [bp+6]
    mov dl, [bp+8]
    call cell_address
    lds si, [bp+10]
    mov cx, [bp+14]
    mov ah, [bp+16]
    cld
    jcxz short written
more:
    lodsb
    stosw
    loop more
written:
    pop es
    pop ds
    pop di
    pop si
    pop bp
    retf
_llrm_os_screen_write endp

; _llrm_os_screen_get(row: u8, column: u8) -> u16
_llrm_os_screen_get proc far
    push bp
    mov bp, sp
    push di
    push es
    mov dh, [bp+6]
    mov dl, [bp+8]
    call cell_address
    mov ax, es:[di]
    pop es
    pop di
    pop bp
    retf
_llrm_os_screen_get endp

; _llrm_os_screen_scroll(top: u8, bottom: u8, lines: u8, attribute: u8)
_llrm_os_screen_scroll proc far
    push bp
    mov bp, sp
    push bx
    push si
    push di
    push ds
    mov ax, BIOS_DATA
    mov ds, ax
    mov dl, ds:[BIOS_COLUMNS]
    pop ds
    dec dl
    mov ch, [bp+6]
    mov dh, [bp+8]
    mov al, [bp+10]
    mov bh, [bp+12]
    xor cl, cl
    mov ah, DOS_VIDEO_SCROLL_UP
    int DOS_VIDEO_INT
    pop di
    pop si
    pop bx
    pop bp
    retf
_llrm_os_screen_scroll endp

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
