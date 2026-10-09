; QB's far file entries around portable channel state and the shared OS layer.
.model medium
.386

extrn _qb_file_attach@3:near
extrn _qb_file_handle@3:near
extrn _qb_file_detach@3:near
extrn _llrm_os_open:far
extrn _llrm_os_create:far
extrn _llrm_os_read:far
extrn _llrm_os_seek:far
extrn _llrm_os_close:far

public B$OPEN
public B$FLOF
public B$GET3
public B$CLOS

RUNTIME_TEXT segment para public 'CODE'
assume ds:DGROUP

; mode, record length, channel, and descriptor are the four Pascal arguments.
B$OPEN proc far
    push bp
    mov bp, sp
    push si
    push di
    push bx
    push es
    mov si, word ptr [bp+12]
    mov cx, word ptr [si]
    cmp cx, 127
    ja short open_done
    mov si, word ptr [si+2]
    lea di, qb_file_name
    push ds
    pop es
    rep movsb
    mov byte ptr [di], 0
    mov bx, word ptr [bp+6]
    cmp bx, 2
    je short create
    cmp bx, 1
    je short read_only
    mov bx, 2
    jmp short open_existing
read_only:
    xor bx, bx
open_existing:
    push bx
    push ds
    push offset qb_file_name
    call far ptr _llrm_os_open
    add sp, 6
    or ax, ax
    jns short opened
    cmp word ptr [bp+6], 1
    je short open_done
create:
    push ds
    push offset qb_file_name
    call far ptr _llrm_os_create
    add sp, 4
    or ax, ax
    js short open_done
opened:
    mov qb_file_handle_value, ax
    cmp word ptr [bp+6], 8
    jne short attach
    pushw 2
    pushw 0
    pushw 0
    push ax
    call far ptr _llrm_os_seek
    add sp, 8
attach:
    mov dx, word ptr qb_file_handle_value
    mov ax, word ptr [bp+10]
    mov cx, word ptr [bp+6]
    call _qb_file_attach@3
open_done:
    pop es
    pop bx
    pop di
    pop si
    pop bp
    retf 8
B$OPEN endp

; Returns a channel's byte length in DX:AX and restores its file position.
B$FLOF proc far
    push bp
    mov bp, sp
    mov ax, word ptr [bp+6]
    call _qb_file_handle@3
    cmp ax, 0ffffh
    je short flof_failed
    mov qb_file_handle_value, ax
    pushw 1
    pushw 0
    pushw 0
    push ax
    call far ptr _llrm_os_seek
    add sp, 8
    or dx, dx
    js short flof_failed
    mov word ptr qb_file_position, ax
    mov word ptr qb_file_position+2, dx
    pushw 2
    pushw 0
    pushw 0
    push word ptr qb_file_handle_value
    call far ptr _llrm_os_seek
    add sp, 8
    mov word ptr qb_file_length, ax
    mov word ptr qb_file_length+2, dx
    pushw 0
    push word ptr qb_file_position+2
    push word ptr qb_file_position
    push word ptr qb_file_handle_value
    call far ptr _llrm_os_seek
    add sp, 8
    mov ax, word ptr qb_file_length
    mov dx, word ptr qb_file_length+2
    jmp short flof_done
flof_failed:
    mov ax, 0ffffh
    mov dx, 0ffffh
flof_done:
    pop bp
    retf 2
B$FLOF endp

; width, 16:16 destination, and channel are the three Pascal arguments.
B$GET3 proc far
    push bp
    mov bp, sp
    mov ax, word ptr [bp+12]
    call _qb_file_handle@3
    cmp ax, 0ffffh
    je short get_done
    push word ptr [bp+6]
    push word ptr [bp+10]
    push word ptr [bp+8]
    push ax
    call far ptr _llrm_os_read
    add sp, 8
get_done:
    pop bp
    retf 8
B$GET3 endp

; count follows the far return, followed by each requested channel.
B$CLOS proc far
    push bp
    mov bp, sp
    push si
    mov cx, word ptr [bp+6]
    or cx, cx
    jnz short close_list
    mov si, 1
close_all:
    mov ax, si
    call _qb_file_detach@3
    cmp ax, 0ffffh
    je short close_next
    push ax
    call far ptr _llrm_os_close
    add sp, 2
close_next:
    inc si
    cmp si, 256
    jb short close_all
    jmp short close_cleanup
close_list:
    lea si, [bp+8]
close_one:
    mov ax, word ptr [si]
    push cx
    call _qb_file_detach@3
    pop cx
    cmp ax, 0ffffh
    je short close_skipped
    push cx
    push ax
    call far ptr _llrm_os_close
    add sp, 2
    pop cx
close_skipped:
    add si, 2
    loop close_one
close_cleanup:
    mov ax, word ptr [bp+6]
    shl ax, 1
    add ax, 2
    mov qb_file_cleanup, ax
    pop si
    pop bp
    pop word ptr qb_file_return
    pop word ptr qb_file_return+2
    add sp, qb_file_cleanup
    jmp dword ptr qb_file_return
B$CLOS endp

RUNTIME_TEXT ends

_DATA segment word public 'DATA'
qb_file_name db 128 dup (0)
qb_file_handle_value dw ?
qb_file_position dd ?
qb_file_length dd ?
qb_file_cleanup dw ?
qb_file_return dw 2 dup (?)
_DATA ends

end
