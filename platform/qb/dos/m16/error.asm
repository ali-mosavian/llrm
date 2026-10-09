; Unhandled QB errors report the caller BASIC frame before exit.
.model medium
.386

extrn _llrm_os_write_file:far
extrn _llrm_os_exit:far

public qb_error_bad_subscript
public qb_module_segment

.data
qb_module_segment dw 0
qb_error_prefix db 13, 10, 'Subscript out of range in line No line number in module '
qb_error_prefix_end label byte
qb_error_middle db ' at address '
qb_error_middle_end label byte
qb_error_hex db 4 dup (0), ':', 4 dup (0), 13, 10
qb_error_hex_digits db '0123456789ABCDEF'
qb_error_module db 8 dup (0)

RUNTIME_TEXT segment para public 'CODE'
assume ds:DGROUP

; BP is B$RDIM's frame; its saved BP names the caller BASIC frame.
qb_error_bad_subscript proc near
    mov si, [bp]
    push word ptr [si+2]
    push word ptr [si+4]
    mov dx, offset qb_error_prefix
    mov cx, offset qb_error_prefix_end - offset qb_error_prefix
    call qb_error_write
    mov ax, qb_module_segment
    mov es, ax
    mov si, 2
    mov di, offset qb_error_module
    mov cx, 8
qb_error_module_copy:
    mov al, es:[si]
    mov [di], al
    inc si
    inc di
    loop qb_error_module_copy
    mov dx, offset qb_error_module
    mov cx, 8
    call qb_error_write
    mov dx, offset qb_error_middle
    mov cx, offset qb_error_middle_end - offset qb_error_middle
    call qb_error_write
    pop dx
    pop ax
    mov di, offset qb_error_hex
    xchg ax, dx
    call qb_error_word_hex
    inc di
    xchg ax, dx
    call qb_error_word_hex
    mov dx, offset qb_error_hex
    mov cx, 11
    call qb_error_write
    push 0
    call far ptr _llrm_os_exit
qb_error_bad_subscript endp

qb_error_word_hex proc near
    push bx
    push cx
    mov cx, 4
qb_error_hex_digit:
    rol ax, 4
    mov bx, ax
    and bx, 15
    mov bl, qb_error_hex_digits[bx]
    mov [di], bl
    inc di
    loop qb_error_hex_digit
    pop cx
    pop bx
    ret
qb_error_word_hex endp

qb_error_write proc near
    push cx
    push ds
    push dx
    push 1
    call far ptr _llrm_os_write_file
    add sp, 8
    ret
qb_error_write endp

RUNTIME_TEXT ends
end
