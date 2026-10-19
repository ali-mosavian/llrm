.model medium
.386

BR_DATA segment word public 'DATA'
BR_DATA ends
DGROUP group BR_DATA
BR_SKYS segment word public 'DATA'
BR_SKYS ends
DGROUP group BR_SKYS
COMMON segment word public 'DATA'
$QB$COMMON label byte
COMMON ends
DGROUP group COMMON
BC_DATA segment word public 'DATA'
$QB$DATA label byte
db 000h,000h,000h,000h,000h,000h
ROWCOUNT% label byte
db 000h,000h
COLUMNCOUNT% label byte
db 000h,000h
ITEMS% label byte
db 000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h
db 000h,000h,000h,000h,000h,000h
BC_DATA ends
DGROUP group BC_DATA
NMALLOC segment word public 'DATA'
NMALLOC ends
DGROUP group NMALLOC
ENMALLOC segment word public 'DATA'
ENMALLOC ends
DGROUP group ENMALLOC
BC_FT segment word public 'DATA'
$QB$FT label byte
BC_FT ends
DGROUP group BC_FT
BC_CN segment word public 'DATA'
$QB$CN label byte
DYNSZ$D4 label byte
db 004h,000h
    dw DYNSZ$D4+4
db 044h,04fh,04eh,045h
BC_CN ends
DGROUP group BC_CN
BC_DS segment word public 'DATA'
$QB$DS label byte
db 000h,000h
db 020h,033h,02ch,020h,035h,000h
db 0ffh,0ffh,001h
BC_DS ends
DGROUP group BC_DS
BC_SAB segment word public 'DATA'
$QB$SAB label byte
BC_SAB ends
DGROUP group BC_SAB
BC_SA segment word public 'DATA'
$QB$SA label byte
    dd $QB$HEADER
BC_SA ends
DGROUP group BC_SA
extern B$CEND:far
extern B$DDIM:far
extern B$PEI2:far
extern B$PESD:far
extern B$PSI2:far
extern B$RDI2:far
.code DYNSZ_CODE
$QB$MAIN proc far
    push si
    push di
L0_0:
    pushw DGROUP
    push offset ROWCOUNT%
    call far ptr B$RDI2
    pushw DGROUP
    push offset COLUMNCOUNT%
    call far ptr B$RDI2
    pushw -2
    push word ptr ROWCOUNT%
    pushw 4
    push word ptr COLUMNCOUNT%
    pushd 131330
    push offset ITEMS%
    call far ptr B$DDIM
    mov di, word ptr ITEMS%+18
    lea si, [edi+edi*4]
    mov bx, word ptr ITEMS%+10
    mov ax, word ptr ITEMS%+2
    mov es, ax
    add si, si
    mov word ptr es:[bx+si-2], 123
    shl di, 2
    add di, di
    mov word ptr es:[bx+di+6], 456
    push word ptr es:[bx+si-2]
    call far ptr B$PSI2
    mov si, word ptr ITEMS%+18
    shl si, 2
    mov ax, word ptr ITEMS%+2
    mov es, ax
    add si, si
    mov bx, word ptr ITEMS%+10
    push word ptr es:[bx+si+6]
    call far ptr B$PEI2
    push offset DYNSZ$D4
    call far ptr B$PESD
    call far ptr B$CEND
$QB$MAIN endp
$QB$STAT proc near
    push bp
    mov bp, sp
L1_1:
    db 000h,000h
$QB$STAT endp
end
