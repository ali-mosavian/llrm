.model medium
.386

public _main
.data
$str1 label byte
db 008h,000h,000h,000h,000h,000h,000h
$str2 label byte
db 008h,000h,004h,000h,004h,000h,062h,06fh,06ch,074h,000h
$str3 label byte
db 008h,000h,004h,000h,004h,000h,067h,065h,061h,072h,000h
$str4 label byte
db 008h,000h,003h,000h,003h,000h,063h,06fh,067h,000h
$str5 label byte
db 008h,000h,003h,000h,003h,000h,070h,069h,06eh,000h
$str6 label byte
db 008h,000h,002h,000h,002h,000h,03ah,020h,000h
$str7 label byte
db 008h,000h,004h,000h,004h,000h,020h,061h,074h,020h,000h
$str8 label byte
db 008h,000h,006h,000h,006h,000h,06eh,06fh,020h,070h,069h,06eh,000h
$str9 label byte
db 008h,000h,00eh,000h,00eh,000h,063h,068h,065h,061h,070h,065h,073h,074h,020h,067h
db 065h,061h,072h,020h,000h
$str10 label byte
db 008h,000h,007h,000h,007h,000h,06eh,06fh,020h,067h,065h,061h,072h,000h
$str11 label byte
db 008h,000h,011h,000h,011h,000h,020h,075h,06eh,064h,065h,072h,020h,032h,030h,02ch
db 020h,066h,069h,072h,073h,074h,020h,000h
$str12 label byte
db 008h,000h,005h,000h,005h,000h,06ch,06fh,077h,03ah,020h,000h
$str13 label byte
db 008h,000h,002h,000h,002h,000h,020h,028h,000h
$str14 label byte
db 008h,000h,001h,000h,001h,000h,029h,000h
$str15 label byte
db 008h,000h,003h,000h,003h,000h,06eh,075h,074h,000h
$str16 label byte
db 008h,000h,006h,000h,006h,000h,020h,070h,061h,072h,074h,073h,000h
extern N$BDRP:far
extern N$BGRW:far
extern N$EBND:far
extern N$PN:far
extern N$PS:far
extern N$PU2:far
extern N$PV:far
extern N$VCMP:far
extern N$VCPY:far
.code CATALOG_TEXT
G$16 proc near
    push bp
    mov bp, sp
    sub sp, 8
    push si
L0_0:
    mov bx, word ptr [bp+4]
    mov ax, ss
    mov word ptr [bp-2], ax
    mov si, word ptr ss:[bx]
    mov word ptr [bp-6], bx
    mov ax, word ptr [si-4]
    mov word ptr [bp-4], ax
    pushd 393217
    push si
    call far ptr N$BGRW
    mov word ptr [bp-8], ax
    add sp, 6
    mov bx, word ptr [bp-6]
    mov word ptr ss:[bx], ax
    mov bx, word ptr [bp-4]
    lea ax, [ebx+ebx*2]
    add ax, ax
    mov word ptr [bp-6], ax
    push word ptr [bp-2]
    push word ptr [bp+6]
    call far ptr N$VCPY
    add sp, 4
    mov bx, word ptr [bp-6]
    mov si, word ptr [bp-8]
    mov word ptr [bx+si], ax
    mov ax, word ptr [bp+8]
    mov word ptr [bx+si+2], ax
    mov ax, word ptr [bp+10]
    mov word ptr [bx+si+4], ax
    pop si
    leave
    ret 8
G$16 endp
_north proc near
    push bp
    mov bp, sp
    sub sp, 26
L1_0:
    mov word ptr [bp-26], offset $str1+6
    lea ax, [bp-26]
    mov word ptr [bp-24], 4
    mov word ptr [bp-22], 4
    mov word ptr [bp-20], offset $str2+6
    mov word ptr [bp-18], DGROUP
    lea bx, [bp-24]
    pushd 2621445
    push bx
    push ax
    call G$16
    mov word ptr [bp-16], 4
    mov word ptr [bp-14], 4
    mov word ptr [bp-12], offset $str3+6
    mov word ptr [bp-10], DGROUP
    lea ax, [bp-16]
    pushd 196638
    push ax
    lea ax, [bp-26]
    push ax
    call G$16
    mov word ptr [bp-8], 3
    mov word ptr [bp-6], 3
    mov word ptr [bp-4], offset $str4+6
    mov word ptr [bp-2], DGROUP
    lea ax, [bp-8]
    pushd 12
    push ax
    lea ax, [bp-26]
    push ax
    call G$16
    mov ax, word ptr [bp-26]
    mov bx, word ptr [bp+4]
    mov word ptr ss:[bx], ax
    mov word ptr [bp-26], 0
    pushw 0
    call far ptr N$BDRP
    add sp, 2
    leave
    ret 2
_north endp
_south proc near
    push bp
    mov bp, sp
    sub sp, 18
L2_0:
    mov word ptr [bp-18], offset $str1+6
    lea ax, [bp-18]
    mov word ptr [bp-16], 4
    mov word ptr [bp-14], 4
    mov word ptr [bp-12], offset $str3+6
    mov word ptr [bp-10], DGROUP
    lea bx, [bp-16]
    pushd 589852
    push bx
    push ax
    call G$16
    mov word ptr [bp-8], 3
    mov word ptr [bp-6], 3
    mov word ptr [bp-4], offset $str5+6
    mov word ptr [bp-2], DGROUP
    lea ax, [bp-8]
    pushd 6553602
    push ax
    lea ax, [bp-18]
    push ax
    call G$16
    mov ax, word ptr [bp-18]
    mov bx, word ptr [bp+4]
    mov word ptr ss:[bx], ax
    mov word ptr [bp-18], 0
    pushw 0
    call far ptr N$BDRP
    add sp, 2
    leave
    ret 2
_south endp
_find proc near
    push bp
    mov bp, sp
    sub sp, 30
    push si
L3_0:
    mov bx, word ptr [bp+6]
    mov ax, ss
    mov word ptr [bp-22], ax
    mov ax, word ptr [bx-4]
    mov word ptr [bp-24], ax
    mov ax, ss
    mov word ptr [bp-26], ax
    xor ax, ax
    mov word ptr [bp-30], ax
    mov ax, word ptr [bp-30]
    cmp ax, word ptr [bp-24]
    jb L3_13
    jmp L3_26
L3_13:
    mov si, word ptr [bx]
    mov cx, word ptr [si-4]
    mov ax, DGROUP
    mov word ptr [bp-16], cx
    mov word ptr [bp-14], cx
    mov word ptr [bp-12], si
    mov word ptr [bp-10], ax
    push word ptr [bp-22]
    push word ptr [bp+10]
    push word ptr [bp-26]
    lea ax, [bp-16]
    push ax
    mov word ptr [bp-18], bx
    call far ptr N$VCMP
    add sp, 8
    or al, al
    je L3_32
L3_23:
    add word ptr [bp-30], 1
    mov bx, word ptr [bp-18]
    add bx, 6
L3_9:
    mov ax, word ptr [bp-30]
    cmp ax, word ptr [bp-24]
    jb L3_13
L3_26:
    mov bx, word ptr [bp+8]
    mov ax, word ptr [bx-4]
    mov word ptr [bp-28], ax
    mov ax, ss
    mov word ptr [bp-30], ax
    xor ax, ax
    mov word ptr [bp-26], ax
    mov ax, word ptr [bp-26]
    cmp ax, word ptr [bp-28]
    jb L3_48
    jmp L3_61
L3_48:
    mov si, word ptr [bx]
    mov cx, word ptr [si-4]
    mov ax, DGROUP
    mov word ptr [bp-8], cx
    mov word ptr [bp-6], cx
    mov word ptr [bp-4], si
    mov word ptr [bp-2], ax
    push word ptr [bp-22]
    push word ptr [bp+10]
    push word ptr [bp-30]
    lea ax, [bp-8]
    push ax
    mov word ptr [bp-20], bx
    call far ptr N$VCMP
    add sp, 8
    or al, al
    je L3_63
L3_58:
    add word ptr [bp-26], 1
    mov bx, word ptr [bp-20]
    add bx, 6
L3_44:
    mov ax, word ptr [bp-26]
    cmp ax, word ptr [bp-28]
    jb L3_48
L3_61:
    mov bx, word ptr [bp+4]
    mov byte ptr ss:[bx], 1
    pop si
    leave
    ret 8
L3_32:
    mov ax, word ptr [bp-30]
    cmp ax, word ptr [bp-24]
    jae L3_42
L3_35:
    add ax, ax
    add ax, word ptr [bp-30]
    add ax, ax
    add ax, word ptr [bp+6]
    mov bx, DGROUP
    mov si, word ptr [bp+4]
    mov byte ptr ss:[si], 0
    mov word ptr ss:[si+2], ax
    mov word ptr ss:[si+4], bx
    pop si
    leave
    ret 8
L3_63:
    mov ax, word ptr [bp-26]
    cmp ax, word ptr [bp-28]
    jae L3_73
L3_66:
    add ax, ax
    add ax, word ptr [bp-26]
    add ax, ax
    add ax, word ptr [bp+8]
    mov bx, DGROUP
    mov si, word ptr [bp+4]
    mov byte ptr ss:[si], 0
    mov word ptr ss:[si+2], ax
    mov word ptr ss:[si+4], bx
    pop si
    leave
    ret 8
L3_42:
    call far ptr N$EBND
L3_73:
    call far ptr N$EBND
_find endp
_affordable proc near
    push bp
    mov bp, sp
    push si
    push di
L4_0:
    mov si, word ptr [bp+4]
    mov bx, word ptr [bp+6]
    mov cx, word ptr [bx-4]
    xor di, di
    xor ax, ax
    cmp ax, cx
    jb L4_14
    jmp L4_10
L4_14:
    cmp word ptr [bx+di+2], 20
    ja L4_10
L4_7:
    inc ax
    add di, 6
L4_3:
    cmp ax, cx
    jb L4_14
L4_10:
    mov dx, DGROUP
    cmp ax, cx
    ja L4_25
L4_19:
    mov word ptr ss:[si], ax
    mov word ptr ss:[si+2], ax
    mov word ptr ss:[si+4], bx
    mov word ptr ss:[si+6], dx
    pop di
    pop si
    pop bp
    ret 4
L4_25:
    call far ptr N$EBND
_affordable endp
_initial proc near
    push bp
    mov bp, sp
    push si
L5_0:
    mov bx, word ptr [bp+4]
    mov si, word ptr [bp+6]
    mov ax, DGROUP
    cmp word ptr [si-4], 1
    jb L5_11
L5_5:
    mov word ptr ss:[bx], 1
    mov word ptr ss:[bx+2], 1
    mov word ptr ss:[bx+4], si
    mov word ptr ss:[bx+6], ax
    pop si
    pop bp
    ret 4
L5_11:
    call far ptr N$EBND
_initial endp
_main proc far
    push bp
    mov bp, sp
    sub sp, 102
    push si
L6_0:
    lea ax, [bp-70]
    push ax
    call _north
    mov ax, word ptr [bp-70]
    mov word ptr [bp-72], ax
    lea ax, [bp-68]
    push ax
    call _south
    mov ax, word ptr [bp-68]
    mov word ptr [bp-80], ax
    lea bx, [bp-66]
    lea si, [bp-72]
    mov word ptr [bp-60], 3
    mov word ptr [bp-58], 3
    mov word ptr [bp-56], offset $str5+6
    mov word ptr [bp-54], DGROUP
    lea dx, [bp-60]
    mov cx, word ptr ss:[si]
    push dx
    push ax
    push cx
    push bx
    call _find
    cmp byte ptr [bp-66], 0
    jne L6_61
L6_65:
    mov bx, word ptr [bp-64]
    mov word ptr [bp-82], bx
    mov es, word ptr [bp-62]
    mov word ptr [bp-84], es
    push word ptr es:[bx]
    call far ptr N$PS
    add sp, 2
    push offset $str6+6
    call far ptr N$PS
    add sp, 2
    mov bx, word ptr [bp-82]
    mov es, word ptr [bp-84]
    push word ptr es:[bx+4]
    call far ptr N$PU2
    add sp, 2
    push offset $str7+6
    call far ptr N$PS
    add sp, 2
    mov bx, word ptr [bp-82]
    mov es, word ptr [bp-84]
    push word ptr es:[bx+2]
    call far ptr N$PU2
    add sp, 2
    call far ptr N$PN
    jmp L6_34
L6_61:
    push offset $str8+6
    call far ptr N$PS
    add sp, 2
    call far ptr N$PN
L6_34:
    lea ax, [bp-52]
    mov word ptr [bp-46], 4
    mov word ptr [bp-44], 4
    mov word ptr [bp-42], offset $str3+6
    mov word ptr [bp-40], DGROUP
    lea cx, [bp-46]
    lea bx, [bp-72]
    mov bx, word ptr ss:[bx]
    mov word ptr [bp-86], bx
    push cx
    push word ptr [bp-80]
    push bx
    push ax
    call _find
    lea ax, [bp-38]
    mov word ptr [bp-32], 4
    mov word ptr [bp-30], 4
    mov word ptr [bp-28], offset $str3+6
    mov word ptr [bp-26], DGROUP
    lea bx, [bp-32]
    push bx
    push word ptr [bp-86]
    push word ptr [bp-80]
    push ax
    call _find
    les bx, dword ptr [bp-50]
    mov cl, byte ptr [bp-38]
    lfs si, dword ptr [bp-36]
    cmp byte ptr [bp-52], 0
    je L6_93
L6_89:
    push offset $str10+6
    call far ptr N$PS
    add sp, 2
    call far ptr N$PN
L6_81:
    lea ax, [bp-24]
    lea bx, [bp-72]
    push word ptr ss:[bx]
    push ax
    call _affordable
    lea si, [bp-24]
    mov ax, word ptr ss:[si]
    lea bx, [bp-16]
    mov cx, ss
    or ax, ax
    je L6_129
L6_110:
    les si, dword ptr ss:[si+4]
    push word ptr es:[si]
    push bx
    mov word ptr [bp-90], cx
    mov word ptr [bp-88], ax
    call _initial
    push word ptr [bp-88]
    call far ptr N$PU2
    add sp, 2
    push offset $str11+6
    call far ptr N$PS
    add sp, 2
    push word ptr [bp-90]
    lea ax, [bp-16]
    push ax
    call far ptr N$PV
    add sp, 4
    call far ptr N$PN
    mov bx, word ptr [bp-72]
    mov cx, word ptr [bx-4]
    mov ax, cx
    neg ax
    or cx, cx
    jbe L6_141
L6_131:
    cmp word ptr [bx+4], 5
    jae L6_137
L6_159:
    push offset $str12+6
    mov word ptr [bp-92], ax
    mov word ptr [bp-78], bx
    call far ptr N$PS
    add sp, 2
    mov bx, word ptr [bp-78]
    push word ptr [bx]
    call far ptr N$PS
    add sp, 2
    push offset $str13+6
    call far ptr N$PS
    add sp, 2
    mov bx, word ptr [bp-78]
    push word ptr [bx+4]
    call far ptr N$PU2
    add sp, 2
    push offset $str14+6
    call far ptr N$PS
    add sp, 2
    call far ptr N$PN
    mov bx, word ptr [bp-78]
    mov ax, word ptr [bp-92]
L6_137:
    add bx, 6
    inc ax
    jne L6_131
L6_141:
    mov word ptr [bp-8], 3
    mov word ptr [bp-6], 3
    mov word ptr [bp-4], offset $str15+6
    mov word ptr [bp-2], DGROUP
    lea ax, [bp-8]
    pushd 32768001
    push ax
    lea ax, [bp-72]
    push ax
    call G$16
    mov bx, word ptr [bp-72]
    push word ptr [bx-4]
    call far ptr N$PU2
    add sp, 2
    push offset $str16+6
    call far ptr N$PS
    add sp, 2
    call far ptr N$PN
    mov bx, word ptr [bp-80]
    cmp bx, 0
    je L6_169
L6_173:
    mov ax, word ptr [bx-4]
    lea cx, [eax+eax*2]
    add cx, cx
    mov si, cx
    neg si
    add cx, bx
    mov word ptr [bp-94], cx
    or ax, ax
    jbe L6_169
L6_181:
    mov word ptr [bp-96], si
    mov bx, word ptr [bp-94]
    push word ptr [bx+si]
    call far ptr N$BDRP
    add sp, 2
    mov si, word ptr [bp-96]
    add si, 6
    jne L6_181
L6_206:
    mov bx, word ptr [bp-80]
L6_169:
    push bx
    call far ptr N$BDRP
    add sp, 2
    mov bx, word ptr [bp-72]
    cmp bx, 0
    je L6_188
L6_190:
    mov ax, word ptr [bx-4]
    lea cx, [eax+eax*2]
    add cx, cx
    mov si, cx
    neg si
    add cx, bx
    mov word ptr [bp-100], cx
    or ax, ax
    jbe L6_188
L6_207:
    mov word ptr [bp-98], bx
L6_198:
    mov word ptr [bp-102], si
    mov bx, word ptr [bp-100]
    push word ptr [bx+si]
    call far ptr N$BDRP
    add sp, 2
    mov si, word ptr [bp-102]
    add si, 6
    jne L6_198
L6_208:
    mov bx, word ptr [bp-98]
L6_188:
    push bx
    call far ptr N$BDRP
    add sp, 2
    xor ax, ax
    pop si
    leave
    retf
L6_93:
    or cl, cl
    jne L6_89
L6_95:
    lea ax, [bx+2]
    mov cx, word ptr es:[bx+2]
    lea bx, [si+2]
    cmp cx, word ptr fs:[si+2]
    jbe L6_101
L6_102:
    mov ax, bx
    mov bx, fs
    jmp L6_103
L6_101:
    mov bx, es
L6_103:
    mov word ptr [bp-74], ax
    mov word ptr [bp-76], bx
    push offset $str9+6
    call far ptr N$PS
    add sp, 2
    mov bx, word ptr [bp-74]
    mov es, word ptr [bp-76]
    push word ptr es:[bx]
    call far ptr N$PU2
    add sp, 2
    call far ptr N$PN
    jmp L6_81
L6_129:
    call far ptr N$EBND
_main endp
end
