.model medium
.386

public N$EDIV
public N$ESTK
public N$ECNV
public N$ESHF
public N$EBND
public N$EKEY
public N$BRES
public N$BDRP
public N$BGRW
public N$BSHR
public N$BCLN
public N$DRES
public N$TAPP
public N$TCAT
public N$VCPY
public N$VCMP
public N$PI1
public N$PU1
public N$PI2
public N$PU2
public N$PI4
public N$PU4
public N$PQ4
public N$PQ2
public N$PB
public N$PC
public N$PS
public N$PV
public N$PN
public N$PFLD
public N$PBEG
public N$PEND
public N$PR8
public N$PR4
.data
    align 2
G$0 label byte
db 000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h
db 000h,000h,000h,000h,000h,000h,000h,000h,000h,000h
    align 2
G$1 label byte
db 000h,000h
    align 2
G$2 label byte
db 000h,000h
G$3 label byte
db 000h
G$4 label byte
db 00ah
G$5 label byte
db 020h
G$6 label byte
db 000h
    align 2
G$7 label byte
db 000h,000h
G$8 label byte
db 000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h
db 000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h
db 000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h
$str10 label byte
db 008h,000h,007h,000h,007h,000h,070h,061h,06eh,069h,063h,03ah,020h,000h
$str11 label byte
db 008h,000h,002h,000h,002h,000h,00dh,00ah,000h
$str12 label byte
db 008h,000h,01ch,000h,01ch,000h,064h,069h,076h,069h,073h,069h,06fh,06eh,020h,062h
db 079h,020h,07ah,065h,072h,06fh,020h,06fh,072h,020h,06fh,076h,065h,072h,066h,06ch
db 06fh,077h,000h
$str13 label byte
db 008h,000h,00eh,000h,00eh,000h,073h,074h,061h,063h,06bh,020h,06fh,076h,065h,072h
db 066h,06ch,06fh,077h,000h
$str14 label byte
db 008h,000h,01eh,000h,01eh,000h,066h,06ch,06fh,061h,074h,020h,06fh,075h,074h,073h
db 069h,064h,065h,020h,074h,068h,065h,020h,069h,06eh,074h,065h,067h,065h,072h,020h
db 074h,079h,070h,065h,000h
$str15 label byte
db 008h,000h,018h,000h,018h,000h,073h,068h,069h,066h,074h,020h,063h,06fh,075h,06eh
db 074h,020h,06fh,075h,074h,020h,06fh,066h,020h,072h,061h,06eh,067h,065h,000h
$str16 label byte
db 008h,000h,013h,000h,013h,000h,069h,06eh,064h,065h,078h,020h,06fh,075h,074h,020h
db 06fh,066h,020h,062h,06fh,075h,06eh,064h,073h,000h
$str17 label byte
db 008h,000h,00dh,000h,00dh,000h,06bh,065h,079h,020h,06eh,06fh,074h,020h,066h,06fh
db 075h,06eh,064h,000h
$str18 label byte
db 008h,000h,00dh,000h,00dh,000h,06fh,075h,074h,020h,06fh,066h,020h,06dh,065h,06dh
db 06fh,072h,079h,000h
$str19 label byte
db 008h,000h,015h,000h,015h,000h,070h,06fh,070h,020h,066h,072h,06fh,06dh,020h,061h
db 06eh,020h,065h,06dh,070h,074h,079h,020h,076h,065h,063h,000h
$str20 label byte
db 008h,000h,004h,000h,004h,000h,074h,072h,075h,065h,000h
$str21 label byte
db 008h,000h,005h,000h,005h,000h,066h,061h,06ch,073h,065h,000h
$str22 label byte
db 008h,000h,003h,000h,003h,000h,069h,06eh,066h,000h
$str23 label byte
db 008h,000h,003h,000h,003h,000h,06eh,061h,06eh,000h
$str24 label byte
db 008h,000h,003h,000h,003h,000h,030h,02eh,030h,000h
$str25 label byte
db 008h,000h,002h,000h,002h,000h,030h,02eh,000h
extern LL$EXIT:far
extern LL$MORE:far
extern LL$WRITE_FILE:far
.code RUNTIME_TEXT
G$25 proc near
    push bp
    mov bp, sp
L0_1:
    mov cx, word ptr [bp+4]
    mov dx, word ptr [bp+6]
    push ax
    push dx
    push cx
    pushw 1
    call far ptr LL$WRITE_FILE
    add sp, 8
    pop bp
    ret
G$25 endp
G$26 proc near
    push bp
    mov bp, sp
    sub sp, 4
L1_1:
    mov dword ptr [bp-4], -1207959552
    mov ax, word ptr [bp-4]
    mov cx, word ptr [bp-2]
    shl ecx, 16
    movzx eax, ax
    or eax, ecx
    leave
    ret
G$26 endp
G$27 proc near
    push bp
    mov bp, sp
    sub sp, 16
    push si
L2_1:
    mov ax, word ptr $str10+2
    mov cx, DGROUP
    mov word ptr [bp-16], ax
    lea dx, [bp-16]
    mov word ptr [bp-14], ax
    mov word ptr [bp-10], cx
    mov word ptr [bp-12], offset $str10+6
    mov si, ss
    push si
    push dx
    call G$28
    add sp, 4
    mov ax, word ptr [bp+4]
    mov cx, word ptr [bp+6]
    push cx
    push ax
    call G$28
    add sp, 4
    mov ax, word ptr $str11+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str11+6
    push si
    push dx
    call G$28
    add sp, 4
    pushw 255
    call far ptr LL$EXIT
    add sp, 2
    pop si
    leave
    ret
G$27 endp
G$28 proc near
    push bp
    mov bp, sp
    sub sp, 4
L3_1:
    mov bx, word ptr [bp+4]
    mov es, word ptr [bp+6]
    mov ax, word ptr es:[bx+4]
    mov cx, word ptr es:[bx+6]
    mov word ptr [bp-4], ax
    mov word ptr [bp-2], cx
    mov cx, word ptr [bp-4]
    mov dx, word ptr [bp-2]
    mov ax, word ptr es:[bx]
    push dx
    push cx
    call G$25
    add sp, 4
    leave
    ret
G$28 endp
N$EDIV proc far
    push bp
    mov bp, sp
    sub sp, 8
L4_1:
    mov ax, word ptr $str12+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str12+6
    mov ax, ss
    push ax
    push dx
    call G$27
    add sp, 4
    leave
    retf
N$EDIV endp
N$ESTK proc far
    push bp
    mov bp, sp
    sub sp, 8
L5_1:
    mov ax, word ptr $str13+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str13+6
    mov ax, ss
    push ax
    push dx
    call G$27
    add sp, 4
    leave
    retf
N$ESTK endp
N$ECNV proc far
    push bp
    mov bp, sp
    sub sp, 8
L6_1:
    mov ax, word ptr $str14+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str14+6
    mov ax, ss
    push ax
    push dx
    call G$27
    add sp, 4
    leave
    retf
N$ECNV endp
N$ESHF proc far
    push bp
    mov bp, sp
    sub sp, 8
L7_1:
    mov ax, word ptr $str15+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str15+6
    mov ax, ss
    push ax
    push dx
    call G$27
    add sp, 4
    leave
    retf
N$ESHF endp
N$EBND proc far
    push bp
    mov bp, sp
    sub sp, 8
L8_1:
    mov ax, word ptr $str16+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str16+6
    mov ax, ss
    push ax
    push dx
    call G$27
    add sp, 4
    leave
    retf
N$EBND endp
N$EKEY proc far
    push bp
    mov bp, sp
    sub sp, 8
L9_1:
    mov ax, word ptr $str17+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str17+6
    mov ax, ss
    push ax
    push dx
    call G$27
    add sp, 4
    leave
    retf
N$EKEY endp
G$35 proc near
L10_1:
    ret
G$35 endp
G$36 proc near
L11_1:
    call G$35
    mov bx, ax
    mov ax, word ptr [bx]
    and ax, -4
    ret
G$36 endp
G$37 proc near
L12_1:
    add ax, 2
    ret
G$37 endp
G$38 proc near
L13_1:
    add ax, 4
    ret
G$38 endp
G$39 proc near
    push bp
    mov bp, sp
    sub sp, 4
L14_1:
    mov word ptr [bp-4], 0
    shr ax, 4
    mov word ptr [bp-2], ax
L14_2:
    cmp word ptr [bp-2], 0
    je L14_4
L14_3:
    inc word ptr [bp-4]
    shr word ptr [bp-2], 1
    jmp L14_2
L14_4:
    mov ax, word ptr [bp-4]
    leave
    ret
G$39 endp
G$40 proc near
    push bp
    mov bp, sp
    sub sp, 12
    push si
L15_1:
    mov cx, ax
    sub cx, -16
    mov si, ax
    jbe L15_4
L15_2:
    mov ax, word ptr $str18+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str18+6
    mov ax, ss
    push ax
    push dx
    call G$27
    add sp, 4
L15_4:
    add si, 5
    and si, -4
    mov word ptr [bp-12], si
    sub si, 8
    jbe L15_6
L15_5:
    mov ax, word ptr [bp-12]
    mov word ptr [bp-10], ax
    jmp L15_7
L15_6:
    mov word ptr [bp-10], 8
L15_7:
    mov ax, word ptr [bp-10]
    pop si
    leave
    ret
G$40 endp
G$41 proc near
    push bp
    mov bp, sp
    sub sp, 6
L16_1:
    mov cx, ax
    mov ax, cx
    call G$37
    mov bx, ax
    mov ax, word ptr [bx]
    mov word ptr [bp-6], ax
    mov ax, cx
    call G$38
    mov bx, ax
    mov ax, word ptr [bx]
    mov word ptr [bp-4], ax
    test ax, ax
    jne L16_3
L16_2:
    mov ax, cx
    call G$36
    call G$39
    mov word ptr [bp-2], ax
    mov bx, ax
    add bx, ax
    mov ax, word ptr [bp-6]
    mov word ptr G$0[bx], ax
    test ax, ax
    je L16_5
    jmp L16_4
L16_3:
    mov ax, word ptr [bp-4]
    call G$37
    mov bx, ax
    mov ax, word ptr [bp-6]
    mov word ptr [bx], ax
L16_4:
    cmp word ptr [bp-6], 0
    je L16_10
    jmp L16_8
L16_5:
    mov cx, word ptr [bp-2]
    mov ax, -2
    rol ax, cl
    and word ptr G$1, ax
    jmp L16_4
L16_8:
    mov ax, word ptr [bp-6]
    call G$38
    mov bx, ax
    mov ax, word ptr [bp-4]
    mov word ptr [bx], ax
L16_10:
    leave
    ret
G$41 endp
G$42 proc near
    push bp
    mov bp, sp
    sub sp, 6
    push si
    push di
L17_1:
    mov bx, ax
    mov si, dx
    mov ax, bx
    call G$35
    mov word ptr [bp-6], ax
    mov ax, bx
    call G$35
    mov di, ax
    mov ax, word ptr [di]
    and ax, 2
    or ax, si
    mov di, word ptr [bp-6]
    mov word ptr [di], ax
    lea ax, [bx+si-2]
    call G$35
    mov di, ax
    mov word ptr [di], si
    mov ax, bx
    add ax, si
    call G$35
    mov di, ax
    and word ptr [di], -3
    mov ax, si
    call G$39
    mov word ptr [bp-4], ax
    mov si, ax
    add si, ax
    mov ax, word ptr G$0[si]
    mov word ptr [bp-2], ax
    mov ax, bx
    call G$37
    mov si, ax
    mov ax, word ptr [bp-2]
    mov word ptr [si], ax
    mov ax, bx
    call G$38
    mov si, ax
    mov word ptr [si], 0
    cmp word ptr [bp-2], 0
    mov dx, bx
    je L17_4
L17_2:
    mov ax, word ptr [bp-2]
    call G$38
    mov bx, ax
    mov word ptr [bx], dx
L17_4:
    mov cx, word ptr [bp-4]
    mov bx, cx
    add bx, cx
    mov word ptr G$0[bx], dx
    mov ax, 1
    shl ax, cl
    or word ptr G$1, ax
    pop di
    pop si
    leave
    ret
G$42 endp
G$43 proc near
    push bp
    mov bp, sp
    sub sp, 6
L18_1:
    mov cx, ax
    mov ax, cx
    call G$39
    mov word ptr [bp-6], ax
    mov bx, ax
    add bx, ax
    mov ax, word ptr G$0[bx]
    mov word ptr [bp-4], ax
L18_2:
    cmp word ptr [bp-4], 0
    je L18_4
L18_3:
    mov ax, word ptr [bp-4]
    call G$36
    sub ax, cx
    jae L18_5
    jmp L18_7
L18_4:
    mov ax, word ptr G$1
    mov cx, word ptr [bp-6]
    mov dx, cx
    sub dx, 16
    jb L18_8
    jmp L18_9
L18_5:
    mov ax, word ptr [bp-4]
    leave
    ret
L18_7:
    mov ax, word ptr [bp-4]
    call G$37
    mov bx, ax
    mov ax, word ptr [bx]
    mov word ptr [bp-4], ax
    jmp L18_2
L18_8:
    mov dx, 2
    shl dx, cl
    dec dx
    not dx
    and ax, dx
    mov word ptr [bp-2], ax
    je L18_10
    jmp L18_12
L18_9:
    call far ptr N$ESHF
L18_10:
    mov ax, 0
    leave
    ret
L18_12:
    mov word ptr [bp-6], 0
L18_13:
    test byte ptr [bp-2], 1
    jne L18_15
L18_14:
    shr word ptr [bp-2], 1
    inc word ptr [bp-6]
    jmp L18_13
L18_15:
    mov ax, word ptr [bp-6]
    mov cx, ax
    sub cx, 13
    jae L18_17
L18_16:
    mov bx, ax
    add bx, ax
    mov ax, word ptr G$0[bx]
    leave
    ret
L18_17:
    call far ptr N$EBND
G$43 endp
G$44 proc near
    push bp
    mov bp, sp
    sub sp, 20
    push si
L19_1:
    mov cx, ax
    add cx, 4
    sub cx, 1024
    mov si, ax
    jbe L19_3
L19_2:
    lea ax, [si+4]
    mov word ptr [bp-16], ax
    jmp L19_4
L19_3:
    mov word ptr [bp-16], 1024
L19_4:
    mov ax, word ptr [bp-16]
    mov word ptr [bp-14], ax
    cmp word ptr G$2, 0
    sete byte ptr [bp-20]
    jne L19_6
L19_5:
    mov word ptr [bp-12], 2
    jmp L19_7
L19_6:
    mov word ptr [bp-12], 0
L19_7:
    mov ax, word ptr [bp-12]
    mov word ptr [bp-10], ax
    add ax, word ptr [bp-14]
    push ax
    call far ptr LL$MORE
    add sp, 2
    mov word ptr [bp-8], ax
    test ax, ax
    jne L19_10
L19_8:
    add si, 4
    mov word ptr [bp-14], si
    add si, word ptr [bp-10]
    push si
    call far ptr LL$MORE
    add sp, 2
    mov word ptr [bp-8], ax
    test ax, ax
    je L19_11
L19_10:
    mov al, byte ptr [bp-20]
    xor al, 1
    mov byte ptr [bp-19], al
    jne L19_14
    jmp L19_15
L19_11:
    pop si
    leave
    ret
L19_14:
    mov ax, word ptr [bp-8]
    mov cx, word ptr G$2
    add cx, 2
    sub ax, cx
    setne byte ptr [bp-19]
L19_15:
    mov al, byte ptr [bp-19]
    mov byte ptr [bp-18], al
    mov al, byte ptr [bp-20]
    mov byte ptr [bp-17], al
    test al, al
    jne L19_17
L19_16:
    mov al, byte ptr [bp-18]
    mov byte ptr [bp-17], al
L19_17:
    cmp byte ptr [bp-17], 0
    je L19_19
L19_18:
    mov ax, word ptr [bp-8]
    mov word ptr [bp-6], ax
    jmp L19_20
L19_19:
    mov ax, word ptr G$2
    mov word ptr [bp-6], ax
L19_20:
    mov ax, word ptr [bp-6]
    mov word ptr [bp-4], ax
    mov ax, word ptr [bp-14]
    mov word ptr [bp-2], ax
    cmp byte ptr [bp-20], 0
    je L19_24
L19_21:
    mov ax, word ptr [bp-4]
    call G$35
    mov bx, ax
    mov word ptr [bx], 2
L19_23:
    cmp byte ptr [bp-18], 0
    jne L19_25
    jmp L19_27
L19_24:
    cmp byte ptr [bp-18], 0
    jne L19_21
    jmp L19_23
L19_25:
    mov ax, word ptr [bp-14]
    add ax, -4
    mov word ptr [bp-2], ax
L19_27:
    mov ax, word ptr [bp-4]
    add ax, word ptr [bp-2]
    mov word ptr G$2, ax
    call G$35
    mov bx, ax
    mov word ptr [bx], 3
    mov ax, word ptr [bp-4]
    call G$35
    mov bx, ax
    mov cx, word ptr [bp-2]
    mov ax, word ptr [bp-4]
    call G$35
    mov si, ax
    mov ax, word ptr [si]
    and ax, 2
    or ax, cx
    or ax, 1
    mov word ptr [bx], ax
    mov ax, word ptr [bp-4]
    add ax, 2
    call G$47
    pop si
    leave
    ret
G$44 endp
G$45 proc near
    push bp
    mov bp, sp
    sub sp, 4
L20_1:
    mov cx, ax
    mov ax, cx
    call G$36
    mov word ptr [bp-4], ax
    sub ax, dx
    sub ax, 8
    jae L20_4
L20_2:
    leave
    ret
L20_4:
    push si
    mov ax, cx
    add ax, dx
    mov word ptr [bp-2], ax
    call G$35
    mov bx, ax
    mov ax, word ptr [bp-4]
    sub ax, dx
    or ax, 3
    mov word ptr [bx], ax
    mov ax, cx
    call G$35
    mov bx, ax
    mov ax, cx
    call G$35
    mov si, ax
    mov ax, word ptr [si]
    and ax, 3
    or ax, dx
    mov word ptr [bx], ax
    mov ax, word ptr [bp-2]
    add ax, 2
    call G$47
    pop si
    leave
    ret
G$45 endp
G$46 proc near
    push bp
    mov bp, sp
    sub sp, 14
L21_1:
    call G$40
    mov word ptr [bp-14], ax
    call G$43
    mov word ptr [bp-12], ax
    test ax, ax
    jne L21_4
L21_2:
    mov ax, word ptr [bp-14]
    call G$44
    mov ax, word ptr [bp-14]
    call G$43
    mov word ptr [bp-12], ax
    test ax, ax
    je L21_5
L21_4:
    push si
    mov ax, word ptr [bp-12]
    call G$41
    mov ax, word ptr [bp-12]
    call G$36
    mov word ptr [bp-10], ax
    mov ax, word ptr [bp-12]
    call G$35
    mov bx, ax
    mov cx, word ptr [bp-10]
    mov ax, word ptr [bp-12]
    call G$35
    mov si, ax
    mov ax, word ptr [si]
    and ax, 2
    or ax, cx
    or ax, 1
    mov word ptr [bx], ax
    mov ax, word ptr [bp-12]
    add ax, word ptr [bp-10]
    call G$35
    mov bx, ax
    or word ptr [bx], 2
    mov ax, word ptr [bp-12]
    mov dx, word ptr [bp-14]
    call G$45
    mov ax, word ptr [bp-12]
    add ax, 2
    pop si
    leave
    ret
L21_5:
    mov ax, word ptr $str18+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str18+6
    mov ax, ss
    push ax
    push dx
    call G$27
    add sp, 4
    jmp L21_4
G$46 endp
G$47 proc near
L22_1:
    test ax, ax
    jne L22_4
L22_2:
    ret
L22_4:
    push bp
    mov bp, sp
    sub sp, 8
    add ax, -2
    mov word ptr [bp-8], ax
    call G$36
    mov word ptr [bp-6], ax
    add ax, word ptr [bp-8]
    mov word ptr [bp-4], ax
    call G$35
    mov bx, ax
    test byte ptr [bx], 1
    jne L22_7
L22_5:
    mov ax, word ptr [bp-4]
    call G$41
    mov cx, word ptr [bp-6]
    mov ax, word ptr [bp-4]
    call G$36
    add ax, cx
    mov word ptr [bp-6], ax
L22_7:
    mov ax, word ptr [bp-8]
    call G$35
    mov bx, ax
    test byte ptr [bx], 2
    jne L22_10
L22_8:
    mov ax, word ptr [bp-8]
    add ax, -2
    call G$35
    mov bx, ax
    mov cx, word ptr [bx]
    mov word ptr [bp-2], cx
    mov ax, word ptr [bp-8]
    sub ax, cx
    mov word ptr [bp-8], ax
    call G$41
    mov ax, word ptr [bp-2]
    add word ptr [bp-6], ax
L22_10:
    mov ax, word ptr [bp-8]
    mov dx, word ptr [bp-6]
    call G$42
    leave
    ret
G$47 endp
G$48 proc near
    push bp
    mov bp, sp
    sub sp, 8
L23_1:
    add ax, -2
    mov word ptr [bp-8], ax
    mov ax, dx
    call G$40
    mov word ptr [bp-6], ax
    mov ax, word ptr [bp-8]
    call G$36
    mov word ptr [bp-4], ax
    sub ax, word ptr [bp-6]
    jae L23_4
L23_2:
    mov ax, word ptr [bp-8]
    add ax, word ptr [bp-4]
    mov word ptr [bp-2], ax
    call G$35
    mov bx, ax
    test byte ptr [bx], 1
    jne L23_5
    jmp L23_8
L23_4:
    mov ax, word ptr [bp-8]
    mov dx, word ptr [bp-6]
    call G$45
    mov al, 1
    leave
    ret
L23_5:
    mov al, 0
    leave
    ret
L23_7:
    push si
    mov ax, word ptr [bp-2]
    call G$41
    mov cx, word ptr [bp-4]
    mov ax, word ptr [bp-2]
    call G$36
    add ax, cx
    mov word ptr [bp-4], ax
    mov ax, word ptr [bp-8]
    call G$35
    mov bx, ax
    mov cx, word ptr [bp-4]
    mov ax, word ptr [bp-8]
    call G$35
    mov si, ax
    mov ax, word ptr [si]
    and ax, 3
    or ax, cx
    mov word ptr [bx], ax
    mov ax, word ptr [bp-8]
    add ax, word ptr [bp-4]
    call G$35
    mov bx, ax
    or word ptr [bx], 2
    pop si
    jmp L23_4
L23_8:
    mov cx, word ptr [bp-4]
    mov ax, word ptr [bp-2]
    call G$36
    add ax, cx
    sub ax, word ptr [bp-6]
    jb L23_5
    jmp L23_7
G$48 endp
G$49 proc near
L24_1:
    add ax, -6
    ret
G$49 endp
G$50 proc near
L25_1:
    add ax, -4
    ret
G$50 endp
G$51 proc near
L26_1:
    add ax, -2
    ret
G$51 endp
G$52 proc near
    push bp
    mov bp, sp
    sub sp, 4
    push si
    push di
L27_1:
    mov word ptr [bp-4], 0
    mov word ptr [bp-2], dx
    mov si, word ptr [bp+4]
    mov cx, word ptr [bp+6]
    mov di, ax
    mov es, cx
L27_2:
    mov ax, word ptr [bp-4]
    sub ax, word ptr [bp-2]
    jae L27_5
L27_3:
    mov bx, word ptr [bp-4]
    mov al, byte ptr es:[bx+si]
    mov byte ptr [bx+di], al
    inc word ptr [bp-4]
    jmp L27_2
L27_5:
    pop di
    pop si
    leave
    ret
G$52 endp
G$53 proc near
    push bp
    mov bp, sp
    sub sp, 2
L28_1:
    test dx, dx
    sete byte ptr [bp-1]
    mov bx, dx
    mov cx, ax
    je L28_3
L28_2:
    mov dx, 0
    mov ax, -23
    div bx
    sub cx, ax
    setbe byte ptr [bp-1]
L28_3:
    mov al, byte ptr [bp-1]
    leave
    ret
G$53 endp
G$54 proc near
    push bp
    mov bp, sp
    sub sp, 10
    push si
    push di
L29_1:
    mov si, ax
    mov di, dx
    mov ax, si
    mov dx, di
    call G$53
    test al, al
    jne L29_4
L29_2:
    mov ax, word ptr $str18+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str18+6
    mov ax, ss
    push ax
    push dx
    call G$27
    add sp, 4
L29_4:
    imul di, si
    add di, 7
    mov ax, di
    call G$46
    add ax, 6
    mov word ptr [bp-10], ax
    call G$49
    mov bx, ax
    mov byte ptr [bx], 1
    mov ax, word ptr [bp-10]
    call G$49
    mov bx, ax
    mov byte ptr [bx+1], 0
    mov ax, word ptr [bp-10]
    call G$50
    mov bx, ax
    mov word ptr [bx], 0
    mov ax, word ptr [bp-10]
    call G$51
    mov bx, ax
    mov word ptr [bx], si
    mov bx, word ptr [bp-10]
    mov byte ptr [bx], 0
    mov ax, word ptr [bp-10]
    pop di
    pop si
    leave
    ret
G$54 endp
N$BRES proc far
    push bp
    mov bp, sp
    sub sp, 20
    push si
    push di
L30_1:
    mov si, ax
    mov di, cx
    mov ax, si
    call G$50
    mov bx, ax
    mov ax, word ptr [bx]
    mov word ptr [bp-18], ax
    mov ax, si
    call G$51
    mov bx, ax
    mov ax, word ptr [bx]
    mov word ptr [bp-16], ax
    mov ax, dx
    sub ax, word ptr [bp-18]
    jae L30_3
L30_2:
    mov ax, word ptr [bp-18]
    mov word ptr [bp-14], ax
    jmp L30_4
L30_3:
    mov word ptr [bp-14], dx
L30_4:
    mov ax, word ptr [bp-14]
    mov word ptr [bp-12], ax
    mov ax, si
    call G$49
    mov bx, ax
    movzx eax, byte ptr [bx]
    and ax, 9
    sub ax, 1
    sete byte ptr [bp-20]
    je L30_8
    jmp L30_7
L30_5:
    mov ax, si
    pop di
    pop si
    leave
    retf
L30_7:
    mov ax, word ptr [bp-16]
    sub ax, -32768
    jb L30_9
    jmp L30_10
L30_8:
    mov ax, word ptr [bp-16]
    sub ax, word ptr [bp-12]
    jae L30_5
    jmp L30_7
L30_9:
    mov ax, word ptr [bp-16]
    mov cx, ax
    add cx, ax
    mov word ptr [bp-10], cx
    jmp L30_11
L30_10:
    mov ax, word ptr [bp-16]
    mov word ptr [bp-10], ax
L30_11:
    mov ax, word ptr [bp-10]
    mov word ptr [bp-8], ax
    mov cx, word ptr [bp-12]
    sub cx, ax
    setb byte ptr [bp-19]
    jae L30_13
L30_12:
    mov ax, word ptr [bp-8]
    mov dx, di
    call G$53
    mov byte ptr [bp-19], al
L30_13:
    cmp byte ptr [bp-19], 0
    je L30_15
L30_14:
    mov ax, word ptr [bp-8]
    mov word ptr [bp-6], ax
    jmp L30_16
L30_15:
    mov ax, word ptr [bp-12]
    mov word ptr [bp-6], ax
L30_16:
    mov ax, word ptr [bp-6]
    mov word ptr [bp-4], ax
    cmp byte ptr [bp-20], 0
    jne L30_21
    jmp L30_19
L30_17:
    mov ax, si
    call G$51
    mov bx, ax
    mov ax, word ptr [bp-4]
    mov word ptr [bx], ax
    mov ax, si
    pop di
    pop si
    leave
    retf
L30_19:
    mov ax, word ptr [bp-4]
    mov dx, di
    call G$54
    mov word ptr [bp-2], ax
    imul di, word ptr [bp-18]
    inc di
    pushw DGROUP
    push si
    mov dx, di
    call G$52
    add sp, 4
    mov ax, word ptr [bp-2]
    call G$50
    mov bx, ax
    mov ax, word ptr [bp-18]
    mov word ptr [bx], ax
    mov ax, si
    call far ptr N$BDRP
    mov ax, word ptr [bp-2]
    pop di
    pop si
    leave
    retf
L30_20:
    lea ax, [si-6]
    mov cx, word ptr [bp-4]
    mov dx, cx
    imul dx, di
    add dx, 7
    call G$48
    test al, al
    jne L30_17
    jmp L30_19
L30_21:
    mov ax, word ptr [bp-4]
    mov dx, di
    call G$53
    test al, al
    jne L30_20
    jmp L30_19
N$BRES endp
N$BDRP proc far
L31_1:
    mov cx, ax
    test cx, cx
    je L31_4
    jmp L31_5
L31_2:
    add cx, -6
    mov ax, cx
    call G$47
L31_4:
    retf
L31_5:
    mov ax, cx
    call G$49
    mov bx, ax
    movzx eax, byte ptr [bx]
    test al, 1
    jne L31_2
    jmp L31_4
N$BDRP endp
N$BGRW proc far
    push bp
    mov bp, sp
    sub sp, 4
    push si
    push di
L32_1:
    mov bx, ax
    mov si, dx
    mov ax, bx
    call G$50
    mov di, ax
    mov dx, word ptr [di]
    mov word ptr [bp-4], dx
    add dx, si
    mov ax, bx
    call far ptr N$BRES
    mov word ptr [bp-2], ax
    call G$50
    mov bx, ax
    add si, word ptr [bp-4]
    mov word ptr [bx], si
    mov ax, word ptr [bp-2]
    pop di
    pop si
    leave
    retf
N$BGRW endp
N$BSHR proc far
    push bp
    mov bp, sp
    sub sp, 10
    push si
    push di
L33_1:
    mov si, ax
    mov ax, si
    call G$50
    mov bx, ax
    mov ax, word ptr [bx]
    mov word ptr [bp-10], ax
    sub ax, dx
    mov di, dx
    jae L33_4
L33_2:
    mov ax, word ptr $str19+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str19+6
    mov ax, ss
    push ax
    push dx
    call G$27
    add sp, 4
L33_4:
    mov ax, si
    call G$50
    mov bx, ax
    mov ax, word ptr [bp-10]
    sub ax, di
    mov word ptr [bx], ax
    mov ax, word ptr [bp-10]
    sub ax, di
    pop di
    pop si
    leave
    retf
N$BSHR endp
N$BCLN proc far
    push bp
    mov bp, sp
    sub sp, 4
    push si
    push di
L34_1:
    mov si, ax
    mov di, dx
    mov ax, si
    call G$50
    mov bx, ax
    mov ax, word ptr [bx]
    mov word ptr [bp-4], ax
    mov dx, di
    call G$54
    mov word ptr [bp-2], ax
    imul di, word ptr [bp-4]
    inc di
    pushw DGROUP
    push si
    mov dx, di
    call G$52
    add sp, 4
    mov ax, word ptr [bp-2]
    call G$50
    mov bx, ax
    mov ax, word ptr [bp-4]
    mov word ptr [bx], ax
    mov ax, word ptr [bp-2]
    pop di
    pop si
    leave
    retf
N$BCLN endp
N$DRES proc far
    push bp
    mov bp, sp
    sub sp, 28
    push si
    push di
L35_1:
    mov si, ax
    mov di, dx
    mov ax, si
    call G$50
    mov bx, ax
    mov ax, word ptr [bx]
    mov word ptr [bp-26], ax
    mov ax, si
    call G$51
    mov bx, ax
    mov ax, word ptr [bx]
    mov word ptr [bp-24], ax
    inc ax
    shl ax, 2
    imul cx, word ptr [bp-26], 3
    sub ax, cx
    ja L35_4
L35_2:
    mov ax, si
    pop di
    pop si
    leave
    retf
L35_4:
    mov ax, word ptr [bp-26]
    sub ax, 8
    jae L35_6
L35_5:
    mov word ptr [bp-22], 8
    jmp L35_7
L35_6:
    mov ax, word ptr [bp-26]
    mov cx, ax
    add cx, ax
    mov word ptr [bp-22], cx
L35_7:
    mov ax, word ptr [bp-22]
    mov word ptr [bp-20], ax
    mov cx, ax
    dec cx
    mov word ptr [bp-18], cx
    mov dx, di
    call G$54
    mov word ptr [bp-16], ax
    mov ax, word ptr [bp-20]
    imul ax, di
    mov word ptr [bp-14], 0
    mov word ptr [bp-12], ax
    mov word ptr [bp-28], si
L35_8:
    mov ax, word ptr [bp-14]
    sub ax, word ptr [bp-12]
    jae L35_11
L35_9:
    mov bx, word ptr [bp-16]
    mov si, word ptr [bp-14]
    mov byte ptr [bx+si], 0
    mov si, word ptr [bp-28]
    inc word ptr [bp-14]
    jmp L35_8
L35_11:
    mov ax, word ptr [bp-26]
    mov word ptr [bp-10], 0
    mov word ptr [bp-8], ax
L35_12:
    mov ax, word ptr [bp-10]
    sub ax, word ptr [bp-8]
    jae L35_15
L35_13:
    mov bx, word ptr [bp-10]
    imul bx, di
    mov ax, si
    add ax, bx
    mov word ptr [bp-6], ax
    mov ax, word ptr [bx+si]
    mov word ptr [bp-4], ax
    test ax, ax
    je L35_16
    jmp L35_18
L35_15:
    mov ax, word ptr [bp-16]
    call G$50
    mov bx, ax
    mov ax, word ptr [bp-20]
    mov word ptr [bx], ax
    mov ax, word ptr [bp-16]
    call G$51
    mov bx, ax
    mov ax, word ptr [bp-24]
    mov word ptr [bx], ax
    mov ax, si
    call far ptr N$BDRP
    mov ax, word ptr [bp-16]
    pop di
    pop si
    leave
    retf
L35_16:
    inc word ptr [bp-10]
    jmp L35_12
L35_18:
    mov ax, word ptr [bp-4]
    and ax, word ptr [bp-18]
    mov word ptr [bp-2], ax
L35_19:
    mov bx, word ptr [bp-16]
    mov ax, word ptr [bp-2]
    mov si, ax
    imul si, di
    cmp word ptr [bx+si], 0
    je L35_21
L35_20:
    mov ax, word ptr [bp-2]
    inc ax
    and ax, word ptr [bp-18]
    mov word ptr [bp-2], ax
    jmp L35_19
L35_21:
    mov ax, word ptr [bp-2]
    imul ax, di
    add ax, word ptr [bp-16]
    mov cx, word ptr [bp-6]
    pushw DGROUP
    push cx
    mov dx, di
    call G$52
    add sp, 4
    inc word ptr [bp-10]
    mov si, word ptr [bp-28]
    jmp L35_12
N$DRES endp
G$61 proc near
    push bp
    mov bp, sp
    sub sp, 4
    push si
L36_1:
    mov cx, ax
    mov si, dx
    mov ax, cx
    call G$50
    mov bx, ax
    mov dx, word ptr [bx]
    mov word ptr [bp-4], dx
    add dx, si
    mov ax, cx
    mov cx, 1
    call far ptr N$BRES
    mov word ptr [bp-2], ax
    add ax, word ptr [bp-4]
    mov cx, word ptr [bp+4]
    mov dx, word ptr [bp+6]
    push dx
    push cx
    mov dx, si
    call G$52
    add sp, 4
    mov ax, word ptr [bp-2]
    call G$50
    mov bx, ax
    mov ax, word ptr [bp-4]
    add ax, si
    mov word ptr [bx], ax
    mov bx, word ptr [bp-2]
    add si, word ptr [bp-4]
    mov byte ptr [bx+si], 0
    mov ax, word ptr [bp-2]
    pop si
    leave
    ret
G$61 endp
N$TAPP proc far
    push si
L37_1:
    mov si, dx
    mov cx, ax
    mov ax, si
    call G$50
    mov bx, ax
    mov dx, word ptr [bx]
    pushw DGROUP
    push si
    mov ax, cx
    call G$61
    add sp, 4
    pop si
    retf
N$TAPP endp
N$TCAT proc far
    push bp
    mov bp, sp
    sub sp, 4
    push si
    push di
L38_1:
    mov di, ax
    mov si, dx
    mov ax, di
    call G$50
    mov bx, ax
    mov cx, word ptr [bx]
    mov word ptr [bp-4], cx
    mov ax, si
    call G$50
    mov bx, ax
    mov ax, cx
    add ax, word ptr [bx]
    mov dx, 1
    call G$54
    mov word ptr [bp-2], ax
    mov dx, word ptr [bp-4]
    pushw DGROUP
    push di
    call G$52
    add sp, 4
    mov ax, word ptr [bp-2]
    call G$50
    mov bx, ax
    mov ax, word ptr [bp-4]
    mov word ptr [bx], ax
    mov ax, word ptr [bp-2]
    mov dx, si
    call far ptr N$TAPP
    pop di
    pop si
    leave
    retf
N$TCAT endp
N$VCPY proc far
    push bp
    mov bp, sp
    sub sp, 6
    push si
    push di
L39_1:
    mov si, word ptr [bp+6]
    mov es, word ptr [bp+8]
    mov ax, word ptr es:[si]
    mov dx, 1
    call G$54
    mov word ptr [bp-6], ax
    mov fs, word ptr [bp+8]
    mov cx, word ptr fs:[si+4]
    mov dx, word ptr fs:[si+6]
    mov word ptr [bp-4], cx
    mov word ptr [bp-2], dx
    mov cx, word ptr [bp-4]
    mov bx, word ptr [bp-2]
    mov dx, word ptr fs:[si]
    push bx
    push cx
    call G$52
    add sp, 4
    mov bx, word ptr [bp-6]
    mov di, word ptr fs:[si]
    mov byte ptr [bx+di], 0
    mov ax, word ptr [bp-6]
    call G$50
    mov bx, ax
    mov ax, word ptr fs:[si]
    mov word ptr [bx], ax
    mov ax, word ptr [bp-6]
    pop di
    pop si
    leave
    retf
N$VCPY endp
N$VCMP proc far
    push bp
    mov bp, sp
    sub sp, 18
    push si
    push di
L40_1:
    mov si, word ptr [bp+6]
    mov fs, word ptr [bp+8]
    mov ax, word ptr fs:[si]
    mov di, word ptr [bp+10]
    mov es, word ptr [bp+12]
    sub ax, word ptr es:[di]
    jae L40_3
L40_2:
    mov ax, word ptr fs:[si]
    mov word ptr [bp-16], ax
    jmp L40_4
L40_3:
    mov ax, word ptr es:[di]
    mov word ptr [bp-16], ax
L40_4:
    mov ax, word ptr [bp-16]
    mov word ptr [bp-14], ax
    mov cx, word ptr fs:[si+4]
    mov dx, word ptr fs:[si+6]
    mov word ptr [bp-8], cx
    mov word ptr [bp-6], dx
    mov cx, word ptr es:[di+4]
    mov dx, word ptr es:[di+6]
    mov word ptr [bp-4], cx
    mov word ptr [bp-2], dx
    mov word ptr [bp-12], 0
    mov word ptr [bp-10], ax
L40_5:
    mov ax, word ptr [bp-12]
    sub ax, word ptr [bp-10]
    jae L40_8
L40_6:
    mov cx, di
    mov di, word ptr [bp-8]
    mov gs, word ptr [bp-6]
    mov bx, word ptr [bp-12]
    mov al, byte ptr gs:[bx+di]
    mov di, word ptr [bp-4]
    mov gs, word ptr [bp-2]
    sub al, byte ptr gs:[bx+di]
    jne L40_9
    jmp L40_10
L40_8:
    mov ax, word ptr fs:[si]
    sub ax, word ptr es:[di]
    je L40_15
    jmp L40_17
L40_9:
    mov si, word ptr [bp-8]
    mov es, word ptr [bp-6]
    mov bx, word ptr [bp-12]
    mov al, byte ptr es:[bx+si]
    mov si, word ptr [bp-4]
    mov es, word ptr [bp-2]
    sub al, byte ptr es:[bx+si]
    jb L40_12
    jmp L40_13
L40_10:
    inc word ptr [bp-12]
    mov di, cx
    jmp L40_5
L40_12:
    mov byte ptr [bp-18], -1
    jmp L40_14
L40_13:
    mov byte ptr [bp-18], 1
L40_14:
    mov al, byte ptr [bp-18]
    pop di
    pop si
    leave
    retf
L40_15:
    mov al, 0
    pop di
    pop si
    leave
    retf
L40_17:
    mov ax, word ptr fs:[si]
    sub ax, word ptr es:[di]
    jae L40_19
L40_18:
    mov byte ptr [bp-17], -1
    jmp L40_20
L40_19:
    mov byte ptr [bp-17], 1
L40_20:
    mov al, byte ptr [bp-17]
    pop di
    pop si
    leave
    retf
N$VCMP endp
G$66 proc near
    push bp
    mov bp, sp
    sub sp, 2
L41_1:
    mov bx, ax
    mov word ptr [bp-2], offset G$8
    lea ax, G$8[bx]
    leave
    ret
G$66 endp
G$67 proc near
    push bp
    mov bp, sp
L42_1:
    cmp word ptr G$7, 0
    mov cx, word ptr [bp+4]
    mov bx, word ptr [bp+6]
    mov dx, ax
    jne L42_3
L42_2:
    push bx
    push cx
    mov ax, dx
    call G$25
    add sp, 4
    jmp L42_4
L42_3:
    mov ax, word ptr G$7
    push bx
    push cx
    call G$61
    add sp, 4
    mov word ptr G$7, ax
L42_4:
    pop bp
    ret
G$67 endp
G$68 proc near
    push bp
    mov bp, sp
    sub sp, 4
L43_1:
    mov bx, word ptr [bp+4]
    mov es, word ptr [bp+6]
    mov ax, word ptr es:[bx+4]
    mov cx, word ptr es:[bx+6]
    mov word ptr [bp-4], ax
    mov word ptr [bp-2], cx
    mov cx, word ptr [bp-4]
    mov dx, word ptr [bp-2]
    mov ax, word ptr es:[bx]
    push dx
    push cx
    call G$67
    add sp, 4
    leave
    ret
G$68 endp
G$69 proc near
    push bp
    mov bp, sp
    sub sp, 6
L44_1:
    mov word ptr [bp-6], offset G$5
    mov word ptr [bp-4], 0
    mov word ptr [bp-2], ax
L44_2:
    mov ax, word ptr [bp-4]
    sub ax, word ptr [bp-2]
    jae L44_5
L44_3:
    mov ax, word ptr [bp-6]
    pushw DGROUP
    push ax
    mov ax, 1
    call G$67
    add sp, 4
    inc word ptr [bp-4]
    jmp L44_2
L44_5:
    leave
    ret
G$69 endp
G$70 proc near
    push bp
    mov bp, sp
    sub sp, 4
L45_1:
    movzx ecx, byte ptr G$3
    sub cx, ax
    jbe L45_3
L45_2:
    movzx ecx, byte ptr G$3
    sub cx, ax
    mov word ptr [bp-4], cx
    jmp L45_4
L45_3:
    mov word ptr [bp-4], 0
L45_4:
    mov ax, word ptr [bp-4]
    mov word ptr [bp-2], ax
    mov byte ptr G$3, 0
    mov byte ptr G$4, 10
    cmp byte ptr G$6, 0
    jne L45_7
L45_5:
    mov ax, word ptr [bp-2]
    call G$69
L45_7:
    mov ax, word ptr [bp-2]
    leave
    ret
G$70 endp
G$71 proc near
L46_1:
    cmp byte ptr G$6, 0
    je L46_4
L46_2:
    call G$69
L46_4:
    mov byte ptr G$5, 32
    mov byte ptr G$6, 0
    ret
G$71 endp
G$72 proc near
    push bp
    mov bp, sp
    sub sp, 2
    push si
L47_1:
    mov si, ax
    mov ax, si
    call G$70
    mov word ptr [bp-2], ax
    mov ax, word ptr [bp+4]
    mov cx, word ptr [bp+6]
    push cx
    push ax
    mov ax, si
    call G$67
    add sp, 4
    mov ax, word ptr [bp-2]
    call G$71
    pop si
    leave
    ret
G$72 endp
G$73 proc near
    push bp
    mov bp, sp
    sub sp, 4
L48_1:
    mov bx, word ptr [bp+4]
    mov es, word ptr [bp+6]
    mov ax, word ptr es:[bx+4]
    mov cx, word ptr es:[bx+6]
    mov word ptr [bp-4], ax
    mov word ptr [bp-2], cx
    mov cx, word ptr [bp-4]
    mov dx, word ptr [bp-2]
    mov ax, word ptr es:[bx]
    push dx
    push cx
    call G$72
    add sp, 4
    leave
    ret
G$73 endp
G$74 proc near
    push bp
    mov bp, sp
    sub sp, 4
L49_1:
    mov word ptr [bp-4], ax
    mov word ptr [bp-2], dx
    mov al, byte ptr G$5
    sub al, 48
    je L49_7
    jmp L49_4
L49_2:
    mov ax, word ptr [bp-4]
    pushw DGROUP
    push ax
    mov ax, 1
    call G$67
    add sp, 4
    inc word ptr [bp-4]
    dec word ptr [bp-2]
    cmp byte ptr G$3, 0
    jne L49_8
L49_4:
    mov cx, word ptr [bp-4]
    mov ax, word ptr [bp-2]
    pushw DGROUP
    push cx
    call G$72
    add sp, 4
    leave
    ret
L49_5:
    mov bx, word ptr [bp-4]
    mov al, byte ptr [bx]
    sub al, 45
    je L49_2
    jmp L49_4
L49_6:
    cmp word ptr [bp-2], 0
    jne L49_5
    jmp L49_4
L49_7:
    cmp byte ptr G$6, 0
    jne L49_4
    jmp L49_6
L49_8:
    dec byte ptr G$3
    jmp L49_4
G$74 endp
G$75 proc near
    push bp
    mov bp, sp
    sub sp, 8
    push si
L50_1:
    mov dword ptr [bp-4], edx
    mov word ptr [bp-6], 0
    mov bx, ax
L50_3:
    mov eax, dword ptr [bp-4]
    movzx esi, cl
    mov edx, 0
    div esi
    mov byte ptr [bp-8], dl
    mov ax, word ptr [bp-6]
    inc ax
    mov word ptr [bp-6], ax
    mov si, bx
    sub si, ax
    movzx ax, dl
    sub ax, 10
    jge L50_5
L50_4:
    mov al, byte ptr [bp-8]
    add al, 48
    mov byte ptr [bp-7], al
    jmp L50_6
L50_5:
    mov al, byte ptr [bp-8]
    add al, 87
    mov byte ptr [bp-7], al
L50_6:
    mov al, byte ptr [bp-7]
    mov byte ptr [si], al
    mov eax, dword ptr [bp-4]
    movzx esi, cl
    mov edx, 0
    div esi
    mov dword ptr [bp-4], eax
    test eax, eax
    jne L50_3
L50_7:
    mov ax, word ptr [bp-6]
    pop si
    leave
    ret
G$75 endp
G$76 proc near
    push bp
    mov bp, sp
    sub sp, 6
    push si
L51_1:
    mov esi, eax
    mov byte ptr [bp-6], dl
    mov ax, 16
    call G$66
    mov word ptr [bp-4], ax
    mov cl, byte ptr G$4
    mov edx, esi
    call G$75
    mov word ptr [bp-2], ax
    mov al, byte ptr [bp-6]
    test al, al
    je L51_4
L51_2:
    mov ax, word ptr [bp-2]
    inc ax
    mov word ptr [bp-2], ax
    mov cx, word ptr [bp-4]
    mov bx, cx
    sub bx, ax
    mov byte ptr [bx], 45
L51_4:
    mov ax, word ptr [bp-4]
    mov dx, word ptr [bp-2]
    sub ax, dx
    call G$74
    pop si
    leave
    ret
G$76 endp
G$77 proc near
    push bp
    mov bp, sp
    sub sp, 4
L52_1:
    test eax, eax
    mov ecx, eax
    jns L52_3
L52_2:
    mov eax, ecx
    neg eax
    mov dword ptr [bp-4], eax
    jmp L52_4
L52_3:
    mov dword ptr [bp-4], ecx
L52_4:
    mov eax, dword ptr [bp-4]
    test ecx, ecx
    sets dl
    call G$76
    leave
    ret
G$77 endp
N$PI1 proc far
L53_1:
    movsx eax, al
    call G$77
    retf
N$PI1 endp
N$PU1 proc far
L54_1:
    movzx eax, al
    mov dl, 0
    call G$76
    retf
N$PU1 endp
N$PI2 proc far
L55_1:
    movsx eax, ax
    call G$77
    retf
N$PI2 endp
N$PU2 proc far
L56_1:
    movzx eax, ax
    mov dl, 0
    call G$76
    retf
N$PU2 endp
N$PI4 proc far
L57_1:
    call G$77
    retf
N$PI4 endp
N$PU4 proc far
L58_1:
    mov dl, 0
    call G$76
    retf
N$PU4 endp
N$PQ4 proc far
    push bp
    mov bp, sp
    sub sp, 30
L59_1:
    mov word ptr [bp-28], 0
    mov dword ptr [bp-16], eax
    test eax, eax
    mov byte ptr [bp-30], dl
    mov ecx, eax
    jns L59_4
L59_2:
    mov ax, 0
    call G$66
    mov bx, ax
    mov byte ptr [bx], 45
    mov word ptr [bp-28], 1
    neg ecx
    mov dword ptr [bp-16], ecx
L59_4:
    mov ax, 16
    call G$66
    mov word ptr [bp-26], ax
    mov edx, dword ptr [bp-16]
    mov cl, byte ptr [bp-30]
    shr edx, cl
    mov cl, 10
    call G$75
    mov word ptr [bp-24], ax
    mov ax, word ptr [bp-28]
    call G$66
    mov cx, word ptr [bp-26]
    mov dx, word ptr [bp-24]
    sub cx, dx
    pushw DGROUP
    push cx
    call G$52
    add sp, 4
    mov ax, word ptr [bp-28]
    add ax, word ptr [bp-24]
    mov word ptr [bp-28], ax
    call G$66
    mov bx, ax
    mov byte ptr [bx], 46
    inc word ptr [bp-28]
    mov eax, 1
    mov cl, byte ptr [bp-30]
    shl eax, cl
    mov dword ptr [bp-12], eax
    dec eax
    and eax, dword ptr [bp-16]
    mov dword ptr [bp-8], eax
    jne L59_7
L59_5:
    mov ax, word ptr [bp-28]
    call G$66
    mov bx, ax
    mov byte ptr [bx], 48
    inc word ptr [bp-28]
L59_7:
    mov ecx, dword ptr [bp-12]
    mov edx, -858993459
    mov eax, ecx
    mul edx
    shr edx, 3
    mov dword ptr [bp-4], edx
    mov eax, edx
    add eax, edx
    lea eax, [eax+eax*4]
    sub ecx, eax
    mov word ptr [bp-22], cx
L59_8:
    cmp dword ptr [bp-8], 0
    je L59_10
L59_9:
    mov word ptr [bp-20], 0
    mov word ptr [bp-18], 1
    jmp L59_11
L59_10:
    mov ax, 0
    call G$66
    mov dx, word ptr [bp-28]
    call G$74
    leave
    retf
L59_11:
    mov ax, word ptr [bp-18]
    sub ax, 9
    jbe L59_14
    jmp L59_13
L59_12:
    mov ax, word ptr [bp-18]
    mov word ptr [bp-20], ax
    inc ax
    mov word ptr [bp-18], ax
    jmp L59_11
L59_13:
    mov eax, dword ptr [bp-8]
    movzx ecx, word ptr [bp-20]
    mov edx, dword ptr [bp-4]
    imul edx, ecx
    sub eax, edx
    mov edx, eax
    add edx, eax
    lea eax, [edx+edx*4]
    imul cx, word ptr [bp-22]
    movzx ecx, cx
    sub eax, ecx
    mov dword ptr [bp-8], eax
    mov ax, word ptr [bp-28]
    call G$66
    mov bx, ax
    mov ax, word ptr [bp-20]
    add al, 48
    mov byte ptr [bx], al
    inc word ptr [bp-28]
    jmp L59_8
L59_14:
    mov eax, dword ptr [bp-8]
    movzx ecx, word ptr [bp-18]
    mov edx, dword ptr [bp-4]
    imul edx, ecx
    imul cx, word ptr [bp-22]
    add cx, 9
    movzx ecx, cx
    imul ecx, ecx, 52429
    shr ecx, 19
    add ecx, edx
    sub eax, ecx
    jae L59_12
    jmp L59_13
N$PQ4 endp
N$PQ2 proc far
L60_1:
    movsx eax, ax
    call far ptr N$PQ4
    retf
N$PQ2 endp
N$PB proc far
    push bp
    mov bp, sp
    sub sp, 10
L61_1:
    test al, al
    je L61_3
L61_2:
    mov word ptr [bp-10], offset $str20+6
    jmp L61_4
L61_3:
    mov word ptr [bp-10], offset $str21+6
L61_4:
    mov bx, word ptr [bp-10]
    mov ax, word ptr [bx-4]
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], bx
    mov ax, ss
    push ax
    push dx
    call G$73
    add sp, 4
    leave
    retf
N$PB endp
N$PC proc far
L62_1:
    mov cl, al
    mov ax, 0
    call G$66
    mov bx, ax
    mov byte ptr [bx], cl
    mov ax, 0
    call G$66
    pushw DGROUP
    push ax
    mov ax, 1
    call G$72
    add sp, 4
    retf
N$PC endp
N$PS proc far
L63_1:
    mov cx, ax
    mov ax, cx
    call G$50
    mov bx, ax
    mov ax, word ptr [bx]
    pushw DGROUP
    push cx
    call G$72
    add sp, 4
    retf
N$PS endp
N$PV proc far
    push bp
    mov bp, sp
    sub sp, 4
L64_1:
    mov bx, word ptr [bp+6]
    mov es, word ptr [bp+8]
    mov ax, word ptr es:[bx+4]
    mov cx, word ptr es:[bx+6]
    mov word ptr [bp-4], ax
    mov word ptr [bp-2], cx
    mov cx, word ptr [bp-4]
    mov dx, word ptr [bp-2]
    mov ax, word ptr es:[bx]
    push dx
    push cx
    call G$72
    add sp, 4
    leave
    retf
N$PV endp
N$PN proc far
    push bp
    mov bp, sp
    sub sp, 8
L65_1:
    mov ax, word ptr $str11+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea dx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str11+6
    mov ax, ss
    push ax
    push dx
    call G$68
    add sp, 4
    leave
    retf
N$PN endp
N$PFLD proc far
    push bp
    mov bp, sp
L66_1:
    mov byte ptr G$3, al
    mov byte ptr G$4, dl
    mov byte ptr G$5, cl
    cmp byte ptr [bp+6], 0
    setne byte ptr G$6
    pop bp
    retf
N$PFLD endp
N$PBEG proc far
L67_1:
    mov ax, 16
    mov dx, 1
    call G$54
    mov word ptr G$7, ax
    retf
N$PBEG endp
N$PEND proc far
    push bp
    mov bp, sp
    sub sp, 2
L68_1:
    mov ax, word ptr G$7
    mov word ptr [bp-2], ax
    mov word ptr G$7, 0
    leave
    retf
N$PEND endp
G$94 proc near
    push bp
    mov bp, sp
    sub sp, 4
    push si
L69_1:
    mov word ptr [bp-4], 0
    mov word ptr [bp-2], 72
    mov bx, ax
L69_2:
    mov ax, word ptr [bp-4]
    sub ax, word ptr [bp-2]
    jge L69_5
L69_3:
    mov ax, word ptr [bp-4]
    mov si, ax
    add si, ax
    mov word ptr [bx+si], 0
    inc word ptr [bp-4]
    jmp L69_2
L69_5:
    mov word ptr [bx], dx
    pop si
    leave
    ret
G$94 endp
G$95 proc near
    push bp
    mov bp, sp
    sub sp, 4
    push si
    push di
L70_1:
    mov word ptr [bp-4], 0
    mov word ptr [bp-2], 72
    mov si, dx
    mov di, ax
L70_2:
    mov ax, word ptr [bp-4]
    sub ax, word ptr [bp-2]
    jge L70_5
L70_3:
    mov ax, word ptr [bp-4]
    mov bx, ax
    add bx, ax
    mov ax, word ptr [bx+si]
    mov word ptr [bx+di], ax
    inc word ptr [bp-4]
    jmp L70_2
L70_5:
    pop di
    pop si
    leave
    ret
G$95 endp
G$96 proc near
    push bp
    mov bp, sp
    sub sp, 8
    push si
    push di
L71_1:
    mov cx, dx
    shr cx, 4
    mov word ptr [bp-8], cx
    and dx, 15
    mov word ptr [bp-6], dx
    mov word ptr [bp-4], 72
    mov bx, ax
L71_2:
    cmp word ptr [bp-4], 0
    je L71_4
L71_3:
    mov ax, word ptr [bp-4]
    dec ax
    mov word ptr [bp-4], ax
    mov si, ax
    add si, ax
    add si, bx
    sub ax, word ptr [bp-8]
    jae L71_5
    jmp L71_6
L71_4:
    cmp word ptr [bp-6], 0
    jne L71_8
    jmp L71_10
L71_5:
    mov ax, word ptr [bp-4]
    sub ax, word ptr [bp-8]
    mov di, ax
    add di, ax
    mov ax, word ptr [bx+di]
    mov word ptr [bp-2], ax
    jmp L71_7
L71_6:
    mov word ptr [bp-2], 0
L71_7:
    mov ax, word ptr [bp-2]
    mov word ptr [si], ax
    jmp L71_2
L71_8:
    mov word ptr [bp-4], 72
    jmp L71_11
L71_10:
    pop di
    pop si
    leave
    ret
L71_11:
    mov ax, word ptr [bp-4]
    sub ax, 1
    jbe L71_13
L71_12:
    mov ax, word ptr [bp-4]
    mov cx, ax
    dec cx
    mov word ptr [bp-4], cx
    mov si, cx
    add si, cx
    mov dx, word ptr [bx+si]
    mov cx, word ptr [bp-6]
    shl dx, cl
    add ax, -2
    mov di, ax
    add di, ax
    mov di, word ptr [bx+di]
    mov al, 16
    sub al, cl
    mov cl, al
    shr di, cl
    or di, dx
    mov word ptr [bx+si], di
    jmp L71_11
L71_13:
    mov cx, word ptr [bp-6]
    shl word ptr [bx], cl
    jmp L71_10
G$96 endp
G$97 proc near
    push bp
    mov bp, sp
    sub sp, 12
    push si
L72_1:
    mov dword ptr [bp-8], 0
    mov word ptr [bp-12], 0
    mov word ptr [bp-10], 72
    mov bx, ax
L72_2:
    mov ax, word ptr [bp-12]
    sub ax, word ptr [bp-10]
    jge L72_5
L72_3:
    mov ax, word ptr [bp-12]
    mov si, ax
    add si, ax
    movzx eax, word ptr [bx+si]
    movzx ecx, dx
    imul ecx, eax
    add ecx, dword ptr [bp-8]
    mov dword ptr [bp-4], ecx
    mov word ptr [bx+si], cx
    movzx eax, word ptr [bp-2]
    mov dword ptr [bp-8], eax
    inc word ptr [bp-12]
    jmp L72_2
L72_5:
    pop si
    leave
    ret
G$97 endp
G$98 proc near
    push bp
    mov bp, sp
    sub sp, 12
    push si
    push di
L73_1:
    mov dword ptr [bp-8], 0
    mov word ptr [bp-12], 0
    mov word ptr [bp-10], 72
    mov si, dx
    mov di, ax
L73_2:
    mov ax, word ptr [bp-12]
    sub ax, word ptr [bp-10]
    jge L73_5
L73_3:
    mov ax, word ptr [bp-12]
    mov bx, ax
    add bx, ax
    movzx eax, word ptr [bx+di]
    movzx ecx, word ptr [bx+si]
    add ecx, eax
    add ecx, dword ptr [bp-8]
    mov dword ptr [bp-4], ecx
    mov word ptr [bx+di], cx
    movzx eax, word ptr [bp-2]
    mov dword ptr [bp-8], eax
    inc word ptr [bp-12]
    jmp L73_2
L73_5:
    pop di
    pop si
    leave
    ret
G$98 endp
G$99 proc near
    push bp
    mov bp, sp
    sub sp, 12
    push si
    push di
L74_1:
    mov dword ptr [bp-8], 0
    mov word ptr [bp-12], 0
    mov word ptr [bp-10], 72
    mov si, dx
    mov di, ax
L74_2:
    mov ax, word ptr [bp-12]
    sub ax, word ptr [bp-10]
    jge L74_5
L74_3:
    mov ax, word ptr [bp-12]
    mov bx, ax
    add bx, ax
    movzx eax, word ptr [bx+di]
    movzx ecx, word ptr [bx+si]
    sub eax, ecx
    sub eax, dword ptr [bp-8]
    mov dword ptr [bp-4], eax
    mov word ptr [bx+di], ax
    mov eax, dword ptr [bp-4]
    shr eax, 16
    and eax, 1
    mov dword ptr [bp-8], eax
    inc word ptr [bp-12]
    jmp L74_2
L74_5:
    pop di
    pop si
    leave
    ret
G$99 endp
G$100 proc near
    push bp
    mov bp, sp
    sub sp, 8
    push si
L75_1:
    mov word ptr [bp-2], 72
    mov word ptr [bp-6], dx
    mov word ptr [bp-8], ax
L75_2:
    cmp word ptr [bp-2], 0
    je L75_4
L75_3:
    mov ax, word ptr [bp-2]
    dec ax
    mov word ptr [bp-2], ax
    mov si, ax
    add si, ax
    mov bx, word ptr [bp-8]
    mov ax, word ptr [bx+si]
    mov bx, word ptr [bp-6]
    sub ax, word ptr [bx+si]
    jne L75_5
    jmp L75_2
L75_4:
    mov al, 0
    pop si
    leave
    ret
L75_5:
    mov ax, word ptr [bp-2]
    mov si, ax
    add si, ax
    mov bx, word ptr [bp-8]
    mov ax, word ptr [bx+si]
    mov bx, word ptr [bp-6]
    sub ax, word ptr [bx+si]
    jae L75_9
L75_8:
    mov byte ptr [bp-3], -1
    jmp L75_10
L75_9:
    mov byte ptr [bp-3], 1
L75_10:
    mov al, byte ptr [bp-3]
    pop si
    leave
    ret
G$100 endp
G$101 proc near
    push bp
    mov bp, sp
    sub sp, 4
L76_1:
    call G$100
    mov byte ptr [bp-3], al
    movsx eax, al
    test ax, ax
    setg byte ptr [bp-2]
    jg L76_3
L76_2:
    mov byte ptr [bp-1], cl
    test cl, cl
    jne L76_4
    jmp L76_5
L76_3:
    mov al, byte ptr [bp-2]
    leave
    ret
L76_4:
    cmp byte ptr [bp-3], 0
    sete byte ptr [bp-1]
L76_5:
    mov al, byte ptr [bp-1]
    mov byte ptr [bp-2], al
    jmp L76_3
G$101 endp
G$102 proc near
    push si
L77_1:
    mov bx, dx
    mov si, ax
    mov byte ptr [bx+si], cl
    mov ax, bx
    inc ax
    pop si
    ret
G$102 endp
G$103 proc near
    push bp
    mov bp, sp
    sub sp, 8
    push si
    push di
L78_1:
    mov word ptr [bp-4], dx
    mov di, word ptr [bp+4]
    mov es, word ptr [bp+6]
    mov cx, word ptr es:[di]
    mov word ptr [bp-6], cx
    mov word ptr [bp-2], 0
    mov word ptr [bp-8], ax
L78_2:
    mov bx, word ptr [bp-2]
    mov ax, bx
    sub ax, word ptr [bp-6]
    jae L78_5
L78_3:
    mov si, word ptr es:[di+4]
    mov fs, word ptr es:[di+6]
    mov dx, word ptr [bp-4]
    mov cl, byte ptr fs:[bx+si]
    mov ax, word ptr [bp-8]
    call G$102
    mov word ptr [bp-4], ax
    inc word ptr [bp-2]
    jmp L78_2
L78_5:
    mov ax, word ptr [bp-4]
    pop di
    pop si
    leave
    ret
G$103 endp
G$104 proc near
    push bp
    mov bp, sp
    sub sp, 26
    push si
    push di
L79_1:
    mov word ptr [bp-22], ax
    mov bx, ax
    add bx, 144
    mov word ptr [bp-20], bx
    mov bx, ax
    add bx, 288
    mov word ptr [bp-18], bx
    mov bx, ax
    add bx, 432
    mov word ptr [bp-16], bx
    add ax, 576
    mov word ptr [bp-14], ax
    test cl, cl
    mov si, word ptr [bp+6]
    mov di, dx
    je L79_3
L79_2:
    mov word ptr [bp-12], 1
    jmp L79_4
L79_3:
    mov word ptr [bp-12], 0
L79_4:
    mov ax, word ptr [bp-12]
    mov word ptr [bp-10], ax
    test di, di
    js L79_6
L79_5:
    mov ax, word ptr [bp-22]
    mov bx, word ptr [bp-10]
    lea dx, [bx+di+1]
    call G$96
    mov ax, word ptr [bp-20]
    mov cx, word ptr [bp-10]
    mov dx, cx
    sub dx, 16
    jb L79_8
    jmp L79_9
L79_6:
    mov ax, word ptr [bp-22]
    mov dx, word ptr [bp-10]
    inc dx
    call G$96
    mov ax, word ptr [bp-20]
    mov dx, 1
    call G$94
    mov ax, word ptr [bp-20]
    mov cx, word ptr [bp-10]
    sub cx, di
    mov dx, cx
    inc dx
    call G$96
    mov ax, word ptr [bp-16]
    mov dx, 1
    call G$94
L79_7:
    mov ax, word ptr [bp-18]
    mov dx, word ptr [bp-16]
    call G$95
    mov ax, word ptr [bp-18]
    mov dx, word ptr [bp-10]
    call G$96
    mov word ptr [bp-8], 0
    jmp L79_11
L79_8:
    mov dx, 2
    shl dx, cl
    call G$94
    mov ax, word ptr [bp-16]
    mov dx, 1
    call G$94
    mov ax, word ptr [bp-16]
    mov dx, di
    call G$96
    jmp L79_7
L79_9:
    call far ptr N$ESHF
L79_11:
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-22]
    call G$95
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-18]
    call G$98
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-20]
    mov cl, byte ptr [bp+4]
    xor cl, 1
    call G$101
    test al, al
    je L79_17
L79_15:
    mov ax, word ptr [bp-20]
    mov dx, 10
    call G$97
    inc word ptr [bp-8]
    jmp L79_11
L79_17:
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-22]
    call G$95
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-18]
    call G$98
    mov ax, word ptr [bp-14]
    mov dx, 10
    call G$97
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-20]
    mov cl, byte ptr [bp+4]
    xor cl, 1
    call G$101
    test al, al
    je L79_21
L79_19:
    mov word ptr [bp-6], 0
    jmp L79_23
L79_21:
    mov ax, word ptr [bp-22]
    mov dx, 10
    call G$97
    mov ax, word ptr [bp-18]
    mov dx, 10
    call G$97
    mov ax, word ptr [bp-16]
    mov dx, 10
    call G$97
    dec word ptr [bp-8]
    jmp L79_17
L79_23:
    mov ax, word ptr [bp-22]
    mov dx, 10
    call G$97
    mov ax, word ptr [bp-18]
    mov dx, 10
    call G$97
    mov ax, word ptr [bp-16]
    mov dx, 10
    call G$97
    mov byte ptr [bp-25], 0
L79_24:
    mov ax, word ptr [bp-22]
    mov dx, word ptr [bp-20]
    call G$100
    movsx eax, al
    test ax, ax
    js L79_26
L79_25:
    mov ax, word ptr [bp-22]
    mov dx, word ptr [bp-20]
    call G$99
    inc byte ptr [bp-25]
    jmp L79_24
L79_26:
    mov ax, word ptr [bp-22]
    mov dx, word ptr [bp-16]
    mov cl, byte ptr [bp+4]
    xor cl, 1
    call G$101
    xor al, 1
    mov byte ptr [bp-24], al
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-22]
    call G$95
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-18]
    call G$98
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-20]
    mov cl, byte ptr [bp+4]
    call G$101
    mov byte ptr [bp-23], al
    cmp byte ptr [bp-24], 0
    jne L79_29
    jmp L79_30
L79_27:
    mov dx, word ptr [bp-6]
    mov cl, byte ptr [bp-25]
    add cl, 48
    mov ax, si
    call G$102
    mov word ptr [bp-6], ax
    jmp L79_23
L79_29:
    cmp byte ptr [bp-24], 0
    jne L79_34
    jmp L79_32
L79_30:
    cmp byte ptr [bp-23], 0
    jne L79_29
    jmp L79_27
L79_31:
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-22]
    call G$95
    mov ax, word ptr [bp-14]
    mov dx, 1
    call G$96
    mov ax, word ptr [bp-14]
    mov dx, word ptr [bp-20]
    call G$100
    movsx eax, al
    test ax, ax
    jns L79_35
    jmp L79_33
L79_32:
    cmp byte ptr [bp-23], 0
    jne L79_38
L79_33:
    mov dx, word ptr [bp-6]
    mov cl, byte ptr [bp-25]
    add cl, 48
    mov ax, si
    call G$102
    mov word ptr [bp-6], ax
    mov cx, word ptr [bp-8]
    mov word ptr [bp-4], ax
    mov word ptr [bp-2], cx
    mov eax, dword ptr [bp-4]
    pop di
    pop si
    leave
    ret
L79_34:
    cmp byte ptr [bp-23], 0
    jne L79_31
    jmp L79_32
L79_35:
    inc byte ptr [bp-25]
    jmp L79_33
L79_38:
    inc byte ptr [bp-25]
    jmp L79_33
G$104 endp
G$105 proc near
    push bp
    mov bp, sp
    sub sp, 64
    push si
    push di
L80_1:
    mov si, ax
    mov word ptr [bp-64], si
    mov word ptr [bp-62], dx
    mov di, cx
    mov ax, 752
    call G$46
    mov word ptr [bp-56], ax
    mov word ptr [bp-54], ax
    add ax, 720
    mov word ptr [bp-52], ax
    mov word ptr [bp-50], 0
    mov ax, si
    dec ax
    mov bx, ax
    add bx, ax
    mov si, word ptr [bp+4]
    mov es, word ptr [bp+6]
    mov dx, word ptr es:[bx+si]
    mov word ptr [bp-48], dx
    mov bx, dx
    mov cl, byte ptr [bp-62]
    shr bx, cl
    and bx, di
    mov word ptr [bp-46], bx
    mov bx, 1
    shl bx, cl
    dec bx
    and bx, dx
    mov word ptr [bp-44], bx
    sete byte ptr [bp-59]
    mov word ptr [bp-42], 0
    mov word ptr [bp-40], ax
    mov cx, word ptr [bp-64]
L80_2:
    mov ax, word ptr [bp-42]
    sub ax, word ptr [bp-40]
    jae L80_5
L80_3:
    mov al, byte ptr [bp-59]
    mov byte ptr [bp-58], al
    test al, al
    jne L80_6
    jmp L80_7
L80_5:
    mov ax, word ptr [bp-48]
    shr ax, 15
    test ax, ax
    jne L80_11
    jmp L80_10
L80_6:
    mov ax, word ptr [bp-42]
    mov bx, ax
    add bx, ax
    cmp word ptr es:[bx+si], 0
    sete byte ptr [bp-58]
L80_7:
    mov al, byte ptr [bp-58]
    mov byte ptr [bp-59], al
    inc word ptr [bp-42]
    jmp L80_2
L80_8:
    mov ax, word ptr [bp-52]
    mov dx, word ptr [bp-50]
    mov word ptr [bp-64], di
    mov di, cx
    mov cl, 45
    call G$102
    mov cx, di
    mov di, word ptr [bp-64]
    mov word ptr [bp-50], ax
L80_10:
    mov ax, word ptr [bp-46]
    sub ax, di
    je L80_13
    jmp L80_14
L80_11:
    mov ax, word ptr [bp-46]
    sub ax, di
    jne L80_8
L80_12:
    cmp byte ptr [bp-59], 0
    jne L80_8
    jmp L80_10
L80_13:
    mov ax, word ptr [bp-52]
    mov dx, word ptr [bp-50]
    cmp byte ptr [bp-59], 0
    jne L80_16
    jmp L80_17
L80_14:
    cmp word ptr [bp-46], 0
    je L80_22
    jmp L80_20
L80_15:
    mov ax, word ptr [bp-52]
    mov dx, word ptr [bp-50]
    call G$74
    mov ax, word ptr [bp-56]
    call G$47
    pop di
    pop si
    leave
    ret
L80_16:
    mov word ptr [bp-38], offset $str22+6
    jmp L80_18
L80_17:
    mov word ptr [bp-38], offset $str23+6
L80_18:
    mov bx, word ptr [bp-38]
    mov cx, word ptr [bx-4]
    mov si, DGROUP
    mov word ptr [bp-16], cx
    lea di, [bp-16]
    mov word ptr [bp-14], cx
    mov word ptr [bp-10], si
    mov word ptr [bp-12], bx
    mov cx, ss
    push cx
    push di
    call G$103
    add sp, 4
    mov word ptr [bp-50], ax
    jmp L80_15
L80_19:
    mov ax, word ptr [bp-52]
    mov dx, word ptr [bp-50]
    mov cx, word ptr $str24+2
    mov bx, DGROUP
    mov word ptr [bp-8], cx
    lea si, [bp-8]
    mov word ptr [bp-6], cx
    mov word ptr [bp-2], bx
    mov word ptr [bp-4], offset $str24+6
    mov cx, ss
    push cx
    push si
    call G$103
    add sp, 4
    mov word ptr [bp-50], ax
    jmp L80_15
L80_20:
    mov ax, word ptr [bp-54]
    mov dx, 0
    call G$94
    mov ax, cx
    dec ax
    mov word ptr [bp-36], 0
    mov word ptr [bp-34], ax
    jmp L80_23
L80_22:
    cmp byte ptr [bp-59], 0
    jne L80_19
    jmp L80_20
L80_23:
    mov ax, word ptr [bp-36]
    sub ax, word ptr [bp-34]
    jae L80_26
L80_24:
    mov di, word ptr [bp-54]
    mov ax, word ptr [bp-36]
    mov bx, ax
    add bx, ax
    mov ax, word ptr es:[bx+si]
    mov word ptr [bx+di], ax
    inc word ptr [bp-36]
    jmp L80_23
L80_26:
    mov ax, cx
    dec ax
    mov bx, ax
    add bx, ax
    add bx, word ptr [bp-54]
    mov ax, word ptr [bp-44]
    cmp word ptr [bp-46], 0
    je L80_28
L80_27:
    mov cx, 1
    mov dx, cx
    mov cl, byte ptr [bp-62]
    shl dx, cl
    mov word ptr [bp-32], dx
    jmp L80_29
L80_28:
    mov word ptr [bp-32], 0
L80_29:
    or ax, word ptr [bp-32]
    mov word ptr [bx], ax
    cmp word ptr [bp-46], 0
    je L80_31
L80_30:
    mov ax, word ptr [bp-46]
    mov word ptr [bp-30], ax
    jmp L80_32
L80_31:
    mov word ptr [bp-30], 1
L80_32:
    mov dx, word ptr [bp-30]
    sub dx, word ptr [bp+8]
    mov word ptr [bp-28], dx
    mov ax, word ptr [bp-52]
    add ax, 16
    mov word ptr [bp-26], ax
    mov ax, word ptr [bp-54]
    mov cx, word ptr [bp-46]
    mov bx, cx
    sub bx, 2
    setae byte ptr [bp-57]
    sub cx, 1
    jbe L80_34
L80_33:
    mov cl, byte ptr [bp-59]
    mov byte ptr [bp-57], cl
L80_34:
    mov cl, byte ptr [bp-57]
    test byte ptr es:[si], 1
    sete ch
    mov bx, word ptr [bp-26]
    movzx si, ch
    push bx
    push si
    call G$104
    add sp, 4
    mov dword ptr [bp-20], eax
    mov bx, word ptr [bp-20]
    mov si, word ptr [bp-18]
    mov word ptr [bp-24], bx
    mov word ptr [bp-22], si
    mov ax, word ptr [bp-52]
    mov dx, word ptr [bp-50]
    mov cx, word ptr [bp-26]
    dec si
    push si
    push bx
    call G$106
    add sp, 4
    mov word ptr [bp-50], ax
    jmp L80_15
G$105 endp
G$106 proc near
    push bp
    mov bp, sp
    sub sp, 38
    push si
    push di
L81_1:
    mov si, ax
    mov word ptr [bp-36], dx
    mov dx, word ptr [bp+6]
    sub dx, -4
    mov bx, word ptr [bp+4]
    mov di, cx
    jge L81_5
    jmp L81_4
L81_2:
    mov ax, word ptr [bp+6]
    test ax, ax
    js L81_6
    jmp L81_7
L81_4:
    mov dx, word ptr [bp-36]
    mov cl, byte ptr [di]
    mov ax, si
    call G$102
    mov word ptr [bp-36], ax
    mov ax, word ptr [bp+4]
    sub ax, 1
    ja L81_31
    jmp L81_33
L81_5:
    mov ax, word ptr [bp+6]
    sub ax, 16
    jl L81_2
    jmp L81_4
L81_6:
    mov dx, word ptr [bp-36]
    mov ax, word ptr $str25+2
    mov cx, DGROUP
    mov word ptr [bp-8], ax
    lea bx, [bp-8]
    mov word ptr [bp-6], ax
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], offset $str25+6
    mov ax, ss
    push ax
    push bx
    mov ax, si
    call G$103
    add sp, 4
    mov word ptr [bp-36], ax
    mov ax, -1
    sub ax, word ptr [bp+6]
    mov word ptr [bp-34], 0
    mov word ptr [bp-32], ax
    jmp L81_9
L81_7:
    mov ax, word ptr [bp+6]
    inc ax
    mov word ptr [bp-26], ax
    mov word ptr [bp-24], 0
    mov word ptr [bp-22], ax
    jmp L81_17
L81_9:
    mov ax, word ptr [bp-34]
    sub ax, word ptr [bp-32]
    jge L81_12
L81_10:
    mov dx, word ptr [bp-36]
    mov ax, si
    mov cl, 48
    call G$102
    mov word ptr [bp-36], ax
    inc word ptr [bp-34]
    jmp L81_9
L81_12:
    mov word ptr [bp-30], 0
    mov ax, word ptr [bp+4]
    mov word ptr [bp-28], ax
L81_13:
    mov ax, word ptr [bp-30]
    sub ax, word ptr [bp-28]
    jae L81_16
L81_14:
    mov dx, word ptr [bp-36]
    mov bx, word ptr [bp-30]
    mov cl, byte ptr [bx+di]
    mov ax, si
    call G$102
    mov word ptr [bp-36], ax
    inc word ptr [bp-30]
    jmp L81_13
L81_16:
    mov ax, word ptr [bp-36]
    pop di
    pop si
    leave
    ret
L81_17:
    mov ax, word ptr [bp-24]
    sub ax, word ptr [bp-22]
    jae L81_20
L81_18:
    mov dx, word ptr [bp-36]
    mov ax, word ptr [bp-24]
    sub ax, bx
    jb L81_21
    jmp L81_22
L81_20:
    mov dx, word ptr [bp-36]
    mov ax, si
    mov cl, 46
    call G$102
    mov cx, word ptr [bp+4]
    mov word ptr [bp-36], ax
    mov ax, cx
    sub ax, word ptr [bp-26]
    jbe L81_24
    jmp L81_26
L81_21:
    mov bx, word ptr [bp-24]
    mov al, byte ptr [bx+di]
    mov byte ptr [bp-38], al
    jmp L81_23
L81_22:
    mov byte ptr [bp-38], 48
L81_23:
    mov cl, byte ptr [bp-38]
    mov ax, si
    call G$102
    mov word ptr [bp-36], ax
    inc word ptr [bp-24]
    mov bx, word ptr [bp+4]
    jmp L81_17
L81_24:
    mov dx, word ptr [bp-36]
    mov ax, si
    mov cl, 48
    call G$102
    pop di
    pop si
    leave
    ret
L81_26:
    mov ax, word ptr [bp-26]
    mov word ptr [bp-20], ax
    mov word ptr [bp-18], cx
L81_27:
    mov ax, word ptr [bp-20]
    sub ax, word ptr [bp-18]
    jae L81_30
L81_28:
    mov dx, word ptr [bp-36]
    mov bx, word ptr [bp-20]
    mov cl, byte ptr [bx+di]
    mov ax, si
    call G$102
    mov word ptr [bp-36], ax
    inc word ptr [bp-20]
    jmp L81_27
L81_30:
    mov ax, word ptr [bp-36]
    pop di
    pop si
    leave
    ret
L81_31:
    mov dx, word ptr [bp-36]
    mov ax, si
    mov cl, 46
    call G$102
    mov word ptr [bp-36], ax
    mov word ptr [bp-16], 1
    mov ax, word ptr [bp+4]
    mov word ptr [bp-14], ax
    jmp L81_34
L81_33:
    mov dx, word ptr [bp-36]
    mov ax, si
    mov cl, 101
    call G$102
    mov word ptr [bp-36], ax
    mov dx, ax
    mov di, word ptr [bp+6]
    test di, di
    js L81_38
    jmp L81_39
L81_34:
    mov ax, word ptr [bp-16]
    sub ax, word ptr [bp-14]
    jae L81_33
L81_35:
    mov dx, word ptr [bp-36]
    mov bx, word ptr [bp-16]
    mov cl, byte ptr [bx+di]
    mov ax, si
    call G$102
    mov word ptr [bp-36], ax
    inc word ptr [bp-16]
    jmp L81_34
L81_38:
    mov byte ptr [bp-37], 45
    jmp L81_40
L81_39:
    mov byte ptr [bp-37], 43
L81_40:
    mov cl, byte ptr [bp-37]
    mov ax, si
    call G$102
    mov word ptr [bp-36], ax
    test di, di
    jns L81_42
L81_41:
    mov ax, di
    neg ax
    mov word ptr [bp-12], ax
    jmp L81_43
L81_42:
    mov word ptr [bp-12], di
L81_43:
    mov ax, word ptr [bp-12]
    mov word ptr [bp-10], ax
    sub ax, 100
    jb L81_46
L81_44:
    mov dx, word ptr [bp-36]
    mov ax, word ptr [bp-10]
    shr ax, 2
    movzx eax, ax
    imul eax, eax, 5243
    shr eax, 17
    mov cl, al
    add cl, 48
    mov ax, si
    call G$102
    mov word ptr [bp-36], ax
L81_46:
    mov dx, word ptr [bp-36]
    movzx eax, word ptr [bp-10]
    imul eax, eax, 52429
    shr eax, 19
    imul ecx, eax, 52429
    shr ecx, 19
    imul cx, cx, 10
    sub ax, cx
    mov cl, al
    add cl, 48
    mov ax, si
    call G$102
    mov dx, ax
    mov word ptr [bp-36], dx
    movzx eax, word ptr [bp-10]
    imul ecx, eax, 52429
    shr ecx, 19
    imul cx, cx, 10
    sub ax, cx
    mov cl, al
    add cl, 48
    mov ax, si
    call G$102
    pop di
    pop si
    leave
    ret
G$106 endp
N$PR8 proc far
    push bp
    mov bp, sp
    sub sp, 12
L82_1:
    fld qword ptr [bp+6]
    fstp qword ptr [bp-8]
    lea ax, [bp-8]
    mov cx, ss
    mov word ptr [bp-10], cx
    mov word ptr [bp-12], ax
    mov ax, word ptr [bp-12]
    mov cx, word ptr [bp-10]
    pushw 1075
    push cx
    push ax
    mov dx, 4
    mov ax, 4
    mov cx, 2047
    call G$105
    add sp, 6
    leave
    retf
N$PR8 endp
N$PR4 proc far
    push bp
    mov bp, sp
    sub sp, 8
L83_1:
    fld dword ptr [bp+6]
    fstp dword ptr [bp-8]
    lea ax, [bp-8]
    mov cx, ss
    mov word ptr [bp-2], cx
    mov word ptr [bp-4], ax
    mov ax, word ptr [bp-4]
    mov cx, word ptr [bp-2]
    pushw 150
    push cx
    push ax
    mov ax, 2
    mov dx, 7
    mov cx, 255
    call G$105
    add sp, 6
    leave
    retf
N$PR4 endp
end
