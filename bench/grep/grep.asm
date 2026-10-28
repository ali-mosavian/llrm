.model medium
.386

public _bench_grep@3
public _main
.data
CONST2 segment word public 'DATA'
_patterns label byte
db 04fh,06ch,069h,076h,065h,072h,07ch,053h,063h,072h,06fh,06fh,067h,065h,07ch,043h
db 06fh,070h,070h,065h,072h,066h,069h,065h,06ch,064h,07ch,044h,06fh,06dh,062h,065h
db 079h,07ch,04dh,061h,072h,06ch,065h,079h,07ch,04dh,069h,063h,061h,077h,062h,065h
db 072h,07ch,048h,065h,065h,070h,07ch,067h,068h,06fh,073h,074h,07ch,067h,065h,06eh
db 074h,06ch,065h,06dh,061h,06eh,07ch,054h,069h,06eh,079h,020h,054h,069h,06dh,07ch
db 000h
CONST2 ends
DGROUP group CONST2
.data?
_queue label byte
    db 80 dup (?)
_nxt label byte
    db 2560 dup (?)
_fail label byte
    db 80 dup (?)
_classof label byte
    db 256 dup (?)
_hit label byte
    db 80 dup (?)
extern _input_read@3:far
extern _report@3:far
.code GREP_TEXT
_bench_grep@3 proc far
    push bp
    mov bp, sp
    sub sp, 1052
    push si
    push di
L0_0:
    mov word ptr [bp-28], ax
    mov word ptr [bp-6], 1
    mov byte ptr [bp-2], 1
    xor bx, bx
    mov si, offset _patterns
    mov dl, 1
    jmp L0_2
L0_21:
    mov byte ptr _hit[bx], dl
    inc dl
    xor bx, bx
    inc si
L0_2:
    mov al, byte ptr [si]
    cmp al, 124
    je L0_21
L0_11:
    test al, al
    je L0_13
L0_17:
    movzx di, al
    mov al, byte ptr _classof[di]
    test al, al
    jne L0_31
L0_40:
    mov ax, word ptr [bp-6]
    mov cx, ax
    inc cx
    mov byte ptr _classof[di], al
    mov word ptr [bp-6], cx
L0_31:
    shl bx, 5
    movzx di, al
    mov al, byte ptr _nxt[bx+di]
    test al, al
    jne L0_45
L0_49:
    mov ax, word ptr [bp-2]
    mov cl, al
    inc cl
    mov byte ptr _nxt[bx+di], al
    mov word ptr [bp-2], cx
L0_45:
    movzx bx, al
    inc si
    jmp L0_2
L0_13:
    cmp word ptr [bp-6], 1
    jle L0_72
L0_15:
    mov cx, word ptr [bp-6]
    dec cx
    xor bx, bx
    mov word ptr [bp-2], 0
    jmp L0_57
L0_64:
    inc bx
    cmp cx, bx
    je L0_54
L0_57:
    mov al, byte ptr _nxt+1[bx]
    test al, al
    je L0_64
L0_68:
    mov si, word ptr [bp-2]
    mov byte ptr _queue[si], al
    inc si
    mov word ptr [bp-2], si
    jmp L0_64
L0_54:
    cmp word ptr [bp-2], 0
    jle L0_72
L0_56:
    xor bx, bx
    jmp L0_76
L0_86:
    mov bx, word ptr [bp-26]
    cmp bx, word ptr [bp-2]
    jge L0_72
L0_76:
    mov al, byte ptr _queue[bx]
    inc bx
    mov word ptr [bp-26], bx
    movzx bx, al
    mov di, bx
    shl di, 5
    mov word ptr [bp-20], bx
    mov cl, byte ptr _fail[bx]
    xor bx, bx
    jmp L0_88
L0_115:
    mov si, word ptr [bp-2]
    mov byte ptr _queue[si], dl
    inc si
    mov word ptr [bp-2], si
    mov di, word ptr [bp-8]
    mov cl, byte ptr [bp-4]
L0_124:
    inc bx
    cmp word ptr [bp-6], bx
    je L0_86
L0_88:
    mov dl, byte ptr _nxt[bx+di]
    movzx si, cl
    shl si, 5
    mov al, byte ptr _nxt[bx+si]
    test dl, dl
    je L0_103
L0_107:
    mov word ptr [bp-8], di
    movzx si, dl
    mov byte ptr _fail[si], al
    mov cl, byte ptr _hit[si]
    mov byte ptr [bp-18], cl
    mov di, word ptr [bp-20]
    mov cl, byte ptr _fail[di]
    mov byte ptr [bp-4], cl
    cmp byte ptr [bp-18], 0
    jne L0_115
L0_119:
    movzx di, al
    mov al, byte ptr _hit[di]
    mov byte ptr _hit[si], al
    jmp L0_115
L0_103:
    mov byte ptr _nxt[bx+di], al
    jmp L0_124
L0_72:
    lea ax, [bp-1052]
    mov dx, 1024
    call far ptr _input_read@3
    test ax, ax
    jle L0_1_1
L0_75:
    xor ecx, ecx
    mov word ptr [bp-4], 0
    mov dword ptr [bp-16], 0
    mov dword ptr [bp-12], 0
    mov word ptr [bp-2], 0
    jmp L0_136
L0_145:
    lea ax, [bp-1052]
    mov dx, 1024
    call far ptr _input_read@3
    test ax, ax
    mov ecx, dword ptr [bp-24]
    jle L0_129
L0_136:
    movzx edx, ax
    add edx, ecx
    mov dword ptr [bp-24], edx
    xor si, si
    jmp L0_148
L0_180:
    movzx edx, bx
    xor bx, bx
L0_1_7:
    mov ax, word ptr [bp-8]
    mov word ptr [bp-4], bx
    add dword ptr [bp-12], edx
    inc ecx
    inc si
    cmp ax, si
    je L0_145
L0_148:
    mov word ptr [bp-8], ax
    movzx bx, byte ptr [bp+si-1052]
    mov di, word ptr [bp-2]
    shl di, 5
    mov ax, bx
    movzx bx, byte ptr _classof[bx]
    movzx bx, byte ptr _nxt[bx+di]
    mov word ptr [bp-2], bx
    xor edx, edx
    mov dl, byte ptr _hit[bx]
    test edx, edx
    je L0_174
L0_1_2:
    mov ebx, dword ptr [bp-16]
    add ebx, ebx
    add ebx, ecx
    add ebx, edx
    and ebx, 536870911
    mov dword ptr [bp-16], ebx
    mov bx, 1
    jmp L0_175
L0_174:
    mov bx, word ptr [bp-4]
L0_175:
    xor edx, edx
    cmp ax, 10
    je L0_180
    jmp L0_1_7
L0_129:
    movzx ecx, word ptr [bp-4]
    mov eax, dword ptr [bp-12]
    add eax, ecx
    mov ecx, dword ptr [bp-16]
    jmp L0_132
L0_1_1:
    xor ecx, ecx
    xor eax, eax
L0_132:
    mov bx, word ptr [bp-28]
    mov dword ptr [bx], ecx
    pop di
    pop si
    leave
    retf
_bench_grep@3 endp
_main proc far
    push bp
    mov bp, sp
    sub sp, 4
L1_0:
    lea ax, [bp-4]
    call far ptr _bench_grep@3
    call far ptr _report@3
    mov eax, dword ptr [bp-4]
    call far ptr _report@3
    xor ax, ax
    leave
    retf
_main endp
end
