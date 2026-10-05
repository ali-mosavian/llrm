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
    enter 8, 0
    push si
L0_0:
    mov bx, word ptr [bp+4]
    mov ax, ss
    mov word ptr [bp-2], ax
    mov si, word ptr ss:[bx]
    mov word ptr [bp-6], bx
    mov ax, word ptr [si-4]
    mov word ptr [bp-4], ax
    pushw 6
    pushw 1
    push si
    call far ptr N$BGRW
    mov word ptr [bp-8], ax
    add sp, 6
    mov bx, word ptr [bp-6]
    mov word ptr ss:[bx], ax
    mov ax, word ptr [bp-4]
    imul ax, ax, 6
    mov word ptr [bp-6], ax
    push word ptr [bp-2]
    push word ptr [bp+6]
    call far ptr N$VCPY
    pop cx
    pop cx
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
_find proc near
    enter 34, 0
    push si
L1_0:
    mov bx, word ptr [bp+6]
    mov ax, ss
    mov word ptr [bp-22], ax
    mov bx, word ptr ss:[bx]
    mov word ptr [bp-24], bx
    mov ax, word ptr [bx-4]
    mov word ptr [bp-26], ax
    mov ax, ss
    mov word ptr [bp-28], ax
    xor cx, cx
    jmp L1_10
L1_14:
    mov si, word ptr [bp-24]
    cmp cx, word ptr [si-4]
    mov word ptr [bp-34], cx
    jae L1_37
L1_27:
    mov si, word ptr [bx]
    mov cx, word ptr [si-4]
    mov ax, DGROUP
    mov word ptr [bp-16], cx
    mov word ptr [bp-14], cx
    mov word ptr [bp-12], si
    mov word ptr [bp-10], ax
    push word ptr [bp-22]
    push word ptr [bp+10]
    push word ptr [bp-28]
    lea ax, [bp-16]
    push ax
    mov word ptr [bp-18], bx
    call far ptr N$VCMP
    mov cx, word ptr [bp-34]
    add sp, 8
    or al, al
    je L1_39
L1_17:
    inc cx
    mov bx, word ptr [bp-18]
    add bx, 6
L1_10:
    cmp cx, word ptr [bp-26]
    jb L1_14
L1_20:
    mov bx, word ptr [bp+8]
    mov bx, word ptr ss:[bx]
    mov word ptr [bp-30], bx
    mov ax, word ptr [bx-4]
    mov word ptr [bp-32], ax
    mov ax, ss
    mov word ptr [bp-34], ax
    xor cx, cx
    jmp L1_52
L1_56:
    mov si, word ptr [bp-30]
    cmp cx, word ptr [si-4]
    mov word ptr [bp-28], cx
    jae L1_74
L1_64:
    mov si, word ptr [bx]
    mov cx, word ptr [si-4]
    mov ax, DGROUP
    mov word ptr [bp-8], cx
    mov word ptr [bp-6], cx
    mov word ptr [bp-4], si
    mov word ptr [bp-2], ax
    push word ptr [bp-22]
    push word ptr [bp+10]
    push word ptr [bp-34]
    lea ax, [bp-8]
    push ax
    mov word ptr [bp-20], bx
    call far ptr N$VCMP
    mov cx, word ptr [bp-28]
    add sp, 8
    or al, al
    je L1_76
L1_59:
    inc cx
    mov bx, word ptr [bp-20]
    add bx, 6
L1_52:
    cmp cx, word ptr [bp-32]
    jb L1_56
L1_62:
    mov bx, word ptr [bp+4]
    mov byte ptr ss:[bx], 1
    pop si
    leave
    ret 8
L1_76:
    mov bx, word ptr [bp-30]
    cmp cx, word ptr [bx-4]
    jae L1_87
L1_80:
    imul cx, cx, 6
    add bx, cx
    mov ax, DGROUP
    mov si, word ptr [bp+4]
    mov byte ptr ss:[si], 0
    mov word ptr ss:[si+2], bx
    mov word ptr ss:[si+4], ax
    pop si
    leave
    ret 8
L1_39:
    mov bx, word ptr [bp-24]
    cmp cx, word ptr [bx-4]
    jae L1_50
L1_43:
    imul cx, cx, 6
    add bx, cx
    mov ax, DGROUP
    mov si, word ptr [bp+4]
    mov byte ptr ss:[si], 0
    mov word ptr ss:[si+2], bx
    mov word ptr ss:[si+4], ax
    pop si
    leave
    ret 8
L1_37:
    call far ptr N$EBND
L1_50:
    call far ptr N$EBND
L1_74:
    call far ptr N$EBND
L1_87:
    call far ptr N$EBND
_find endp
_main proc far
    enter 136, 0
L2_0:
    mov word ptr [bp-44], offset $str1+6
    lea ax, [bp-44]
    mov bx, 4
    mov word ptr [bp-42], bx
    mov word ptr [bp-40], bx
    mov word ptr [bp-38], offset $str2+6
    mov word ptr [bp-36], DGROUP
    lea bx, [bp-42]
    pushw 40
    pushw 5
    push bx
    push ax
    call G$16
    mov ax, 4
    mov word ptr [bp-34], ax
    mov word ptr [bp-32], ax
    mov word ptr [bp-30], offset $str3+6
    mov word ptr [bp-28], DGROUP
    lea ax, [bp-34]
    pushw 3
    pushw 30
    push ax
    lea ax, [bp-44]
    push ax
    call G$16
    mov ax, 3
    mov word ptr [bp-26], ax
    mov word ptr [bp-24], ax
    mov word ptr [bp-22], offset $str4+6
    mov word ptr [bp-20], DGROUP
    lea ax, [bp-26]
    pushd 12
    push ax
    lea ax, [bp-44]
    push ax
    call G$16
    mov ax, word ptr [bp-44]
    mov word ptr [bp-114], ax
    mov word ptr [bp-44], 0
    pushw 0
    call far ptr N$BDRP
    pop cx
    mov ax, word ptr [bp-114]
    mov word ptr [bp-106], ax
    mov word ptr [bp-18], offset $str1+6
    lea ax, [bp-18]
    mov bx, 4
    mov word ptr [bp-16], bx
    mov word ptr [bp-14], bx
    mov word ptr [bp-12], offset $str3+6
    mov word ptr [bp-10], DGROUP
    lea bx, [bp-16]
    pushw 9
    pushw 28
    push bx
    push ax
    call G$16
    mov ax, 3
    mov word ptr [bp-8], ax
    mov word ptr [bp-6], ax
    mov word ptr [bp-4], offset $str5+6
    mov word ptr [bp-2], DGROUP
    lea ax, [bp-8]
    pushw 100
    pushw 2
    push ax
    lea ax, [bp-18]
    push ax
    call G$16
    mov ax, word ptr [bp-18]
    mov word ptr [bp-116], ax
    mov word ptr [bp-18], 0
    pushw 0
    call far ptr N$BDRP
    pop cx
    mov ax, word ptr [bp-116]
    mov word ptr [bp-104], ax
    lea ax, [bp-102]
    lea bx, [bp-106]
    lea cx, [bp-104]
    mov dx, 3
    mov word ptr [bp-96], dx
    mov word ptr [bp-94], dx
    mov word ptr [bp-92], offset $str5+6
    mov word ptr [bp-90], DGROUP
    lea dx, [bp-96]
    push dx
    push cx
    push bx
    push ax
    call _find
    cmp byte ptr [bp-102], 0
    jne L2_110
L2_114:
    mov bx, word ptr [bp-100]
    mov word ptr [bp-118], bx
    mov es, word ptr [bp-98]
    mov word ptr [bp-120], es
    push word ptr es:[bx]
    call far ptr N$PS
    pop cx
    push offset $str6+6
    call far ptr N$PS
    pop cx
    mov bx, word ptr [bp-118]
    mov es, word ptr [bp-120]
    push word ptr es:[bx+4]
    call far ptr N$PU2
    pop cx
    push offset $str7+6
    call far ptr N$PS
    pop cx
    mov bx, word ptr [bp-118]
    mov es, word ptr [bp-120]
    push word ptr es:[bx+2]
    call far ptr N$PU2
    pop cx
    call far ptr N$PN
    jmp L2_86
L2_110:
    push offset $str8+6
    call far ptr N$PS
    pop cx
    call far ptr N$PN
L2_86:
    push si
    lea ax, [bp-88]
    mov bx, 4
    mov word ptr [bp-82], bx
    mov word ptr [bp-80], bx
    mov word ptr [bp-78], offset $str3+6
    mov word ptr [bp-76], DGROUP
    lea bx, [bp-82]
    push bx
    lea bx, [bp-104]
    push bx
    lea bx, [bp-106]
    push bx
    push ax
    call _find
    lea ax, [bp-74]
    mov bx, 4
    mov word ptr [bp-68], bx
    mov word ptr [bp-66], bx
    mov word ptr [bp-64], offset $str3+6
    mov word ptr [bp-62], DGROUP
    lea bx, [bp-68]
    push bx
    lea bx, [bp-106]
    push bx
    lea bx, [bp-104]
    push bx
    push ax
    call _find
    les bx, dword ptr [bp-86]
    mov cl, byte ptr [bp-74]
    lfs si, dword ptr [bp-72]
    cmp byte ptr [bp-88], 0
    je L2_162
L2_158:
    push offset $str10+6
    call far ptr N$PS
    pop cx
    call far ptr N$PN
L2_130:
    lea bx, [bp-106]
    mov bx, word ptr ss:[bx]
    mov cx, word ptr [bx-4]
    xor ax, ax
    mov si, bx
    jmp L2_134
L2_144:
    cmp word ptr [si+2], 20
    ja L2_141
L2_138:
    inc ax
    add si, 6
L2_134:
    cmp ax, cx
    jb L2_144
L2_141:
    cmp ax, cx
    ja L2_152
L2_148:
    lea si, [bp-60]
    mov cx, ss
    or ax, ax
    je L2_207
L2_179:
    mov bx, word ptr [bx]
    mov dx, DGROUP
    cmp word ptr [bx-4], 1
    jb L2_205
L2_186:
    mov word ptr ss:[si], 1
    mov word ptr ss:[si+2], 1
    mov word ptr ss:[si+4], bx
    mov word ptr ss:[si+6], dx
    push ax
    mov word ptr [bp-122], cx
    call far ptr N$PU2
    pop cx
    push offset $str11+6
    call far ptr N$PS
    pop cx
    push word ptr [bp-122]
    lea ax, [bp-60]
    push ax
    call far ptr N$PV
    pop cx
    pop cx
    call far ptr N$PN
    mov bx, word ptr [bp-106]
    mov cx, word ptr [bx-4]
    mov ax, cx
    neg ax
    or cx, cx
    jbe L2_219
L2_209:
    cmp word ptr [bx+4], 5
    jae L2_215
L2_238:
    push offset $str12+6
    mov word ptr [bp-124], ax
    mov word ptr [bp-112], bx
    call far ptr N$PS
    pop cx
    mov bx, word ptr [bp-112]
    push word ptr [bx]
    call far ptr N$PS
    pop cx
    push offset $str13+6
    call far ptr N$PS
    pop cx
    mov bx, word ptr [bp-112]
    push word ptr [bx+4]
    call far ptr N$PU2
    pop cx
    push offset $str14+6
    call far ptr N$PS
    pop cx
    call far ptr N$PN
    mov bx, word ptr [bp-112]
    mov ax, word ptr [bp-124]
L2_215:
    add bx, 6
    inc ax
    jne L2_209
L2_219:
    mov ax, 3
    mov word ptr [bp-52], ax
    mov word ptr [bp-50], ax
    mov word ptr [bp-48], offset $str15+6
    mov word ptr [bp-46], DGROUP
    lea ax, [bp-52]
    pushw 500
    pushw 1
    push ax
    lea ax, [bp-106]
    push ax
    call G$16
    mov bx, word ptr [bp-106]
    push word ptr [bx-4]
    call far ptr N$PU2
    pop cx
    push offset $str16+6
    call far ptr N$PS
    pop cx
    call far ptr N$PN
    mov bx, word ptr [bp-104]
    cmp bx, 0
    jne L2_252
L2_248:
    push bx
    call far ptr N$BDRP
    pop cx
    mov bx, word ptr [bp-106]
    cmp bx, 0
    jne L2_269
L2_267:
    push bx
    call far ptr N$BDRP
    pop cx
    xor ax, ax
    pop si
    leave
    retf
L2_269:
    mov ax, word ptr [bx-4]
    imul cx, ax, 6
    mov si, cx
    neg si
    add cx, bx
    mov word ptr [bp-134], cx
    or ax, ax
    jbe L2_267
L2_286:
    mov word ptr [bp-132], bx
L2_277:
    mov word ptr [bp-136], si
    mov bx, word ptr [bp-134]
    push word ptr [bx+si]
    call far ptr N$BDRP
    pop cx
    mov si, word ptr [bp-136]
    add si, 6
    jne L2_277
L2_287:
    mov bx, word ptr [bp-132]
    jmp L2_267
L2_252:
    mov ax, word ptr [bx-4]
    imul cx, ax, 6
    mov si, cx
    neg si
    add cx, bx
    mov word ptr [bp-128], cx
    or ax, ax
    jbe L2_248
L2_284:
    mov word ptr [bp-126], bx
L2_260:
    mov word ptr [bp-130], si
    mov bx, word ptr [bp-128]
    push word ptr [bx+si]
    call far ptr N$BDRP
    pop cx
    mov si, word ptr [bp-130]
    add si, 6
    jne L2_260
L2_285:
    mov bx, word ptr [bp-126]
    jmp L2_248
L2_162:
    or cl, cl
    jne L2_158
L2_164:
    lea ax, [bx+2]
    mov cx, word ptr es:[bx+2]
    lea bx, [si+2]
    cmp cx, word ptr fs:[si+2]
    jbe L2_170
L2_171:
    mov ax, bx
    mov bx, fs
    jmp L2_172
L2_170:
    mov bx, es
L2_172:
    mov word ptr [bp-108], ax
    mov word ptr [bp-110], bx
    push offset $str9+6
    call far ptr N$PS
    pop cx
    mov bx, word ptr [bp-108]
    mov es, word ptr [bp-110]
    push word ptr es:[bx]
    call far ptr N$PU2
    pop cx
    call far ptr N$PN
    jmp L2_130
L2_152:
    call far ptr N$EBND
L2_205:
    call far ptr N$EBND
L2_207:
    call far ptr N$EBND
_main endp
end
