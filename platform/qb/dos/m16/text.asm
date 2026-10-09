; QB45 string descriptors are near: length followed by the near data address.
; These entry points own the real-mode ABI and delegate output to the shared
; operating-system layer.
.model medium
.386

extrn _llrm_os_write_file:far

public B$STI4
public B$LTRM
public B$PESD

.data
qb_temp_descriptor dw 0, offset qb_temp_data
qb_temp_data db 12 dup (0)
qb_newline db 13, 10

QB_TEXT segment para public 'CODE'
; STR$(long): the long is low word then high word.  QB prefixes a positive
; number with a blank and returns a descriptor in AX.
B$STI4 proc far
    push bp
    mov bp, sp
    movzx eax, word ptr [bp+6]
    movzx edx, word ptr [bp+8]
    shl edx, 16
    or eax, edx
    mov di, offset qb_temp_data+12
    xor si, si
    test eax, eax
    jns short STI4_digits
    neg eax
    mov si, 1
STI4_digits:
    mov ebx, 10
STI4_digit:
    xor edx, edx
    div ebx
    add dl, '0'
    dec di
    mov [di], dl
    test eax, eax
    jnz short STI4_digit
    or si, si
    jz short STI4_positive
    dec di
    mov byte ptr [di], '-'
    jmp short STI4_done
STI4_positive:
    dec di
    mov byte ptr [di], ' '
STI4_done:
    mov ax, offset qb_temp_data+12
    sub ax, di
    mov qb_temp_descriptor, ax
    mov qb_temp_descriptor+2, di
    mov ax, offset qb_temp_descriptor
    pop bp
    retf 4
B$STI4 endp

; LTRIM$(descriptor): move the descriptor's window over leading ASCII blanks.
B$LTRM proc far
    push bp
    mov bp, sp
    push si
    mov bx, [bp+6]
    mov cx, [bx]
    mov si, [bx+2]
LTRM_scan:
    jcxz short LTRM_done
    cmp byte ptr [si], ' '
    jne short LTRM_done
    inc si
    dec cx
    jmp short LTRM_scan
LTRM_done:
    mov [bx], cx
    mov [bx+2], si
    mov ax, bx
    pop si
    pop bp
    retf 2
B$LTRM endp

; PRINT descriptor followed by an end-of-line.  The shared layer owns DOS I/O.
B$PESD proc far
    push bp
    mov bp, sp
    mov bx, [bp+6]
    mov cx, [bx]
    mov dx, [bx+2]
    push cx
    push ds
    push dx
    push 1
    call far ptr _llrm_os_write_file
    add sp, 8
    push 2
    push ds
    push offset qb_newline
    push 1
    call far ptr _llrm_os_write_file
    add sp, 8
    pop bp
    retf 2
B$PESD endp

QB_TEXT ends
end
