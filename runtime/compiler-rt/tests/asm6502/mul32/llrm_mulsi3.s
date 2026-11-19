; ours (v2y structure, absolute addressing: __sa/__sb are BSS here) in llrm's runtime slot: __sa * __sb -> __sa (low 32 bits).
; Clobbers A, X, Y, __sa, __sb, __st, as mos_rt.s's own __mulsi3 does. Replaces that routine in the verdict build.
__mulsi3:
    lda #0
    sta __st
    sta __st+1
    sta __st+2
    tay
    tax
by: lda __sb,x
    beq mv
    sec
    ror a
    sta __st+3          ; sentinel cell
bt: bcc nb
    clc
    lda __st
    adc __sa
    sta __st
    lda __st+1
    adc __sa+1
    sta __st+1
    lda __st+2
    adc __sa+2
    sta __st+2
    tya
    adc __sa+3
    tay
nb: asl __sa
    rol __sa+1
    rol __sa+2
    rol __sa+3
    lsr __st+3
    bne bt
    beq nx
mv: lda __sa+2
    sta __sa+3
    lda __sa+1
    sta __sa+2
    lda __sa
    sta __sa+1
    lda #0
    sta __sa
nx: inx
    cpx #4
    bne by
    sty __sa+3
    lda __st
    sta __sa
    lda __st+1
    sta __sa+1
    lda __st+2
    sta __sa+2
    rts
