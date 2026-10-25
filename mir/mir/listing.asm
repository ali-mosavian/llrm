.386
.model flat

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
db 000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h,000h
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
H$D3 label byte
db 005h,000h,000h,000h
    dd H$D3+8
db 048h,045h,04ch,04ch,04fh,000h
BC_CN ends
DGROUP group BC_CN
BC_DS segment word public 'DATA'
$QB$DS label byte
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
extern B$CEND:near
extern B$PESD:near
.code H_CODE
$QB$MAIN proc near
L0_0:
    push offset H$D3
    call B$PESD
    call B$CEND
$QB$MAIN endp
$QB$STAT proc near
    push ebp
    mov ebp, esp
L1_1:
    db 000h,000h,000h,000h
$QB$STAT endp
end
