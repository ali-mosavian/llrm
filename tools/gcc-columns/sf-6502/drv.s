; gcc-ABI driver: OP over vec (records: a, b, expected; 4 bytes each). a in rc2..rc5, b in rc6..rc9 (BIN only), result rc2..rc5.
; Exit code = failures (max 255). Without EXACT an expected canonical NaN passes any NaN, as in tests/asm6502/drv.s.
.export _main
.include "vec.inc"
.include "op.inc"
ZP = 26
.repeat ZP, i
  .exportzp .ident(.concat("rc", .string(i)))
.endrepeat
.segment "ZEROPAGE"
.repeat ZP, i
  .ident(.concat("rc", .string(i))): .res 1
.endrepeat
ptr: .res 2
cnt: .res 2
fail: .res 1
.segment "CODE"
_main:
    lda #0
    sta rc0
    sta fail
    lda #$80
    sta rc1
    lda #<vec
    sta ptr
    lda #>vec
    sta ptr+1
    lda #<nrec
    sta cnt
    lda #>nrec
    sta cnt+1
loop:
.ifdef BIN
    ldy #7
.else
    ldy #3
.endif
:   lda (ptr),y
    sta rc2,y
    dey
    bpl :-
    jsr OP
.ifndef EXACT
    ldy #11
    lda (ptr),y
    cmp #$7f
    bne exact
    dey
    lda (ptr),y
    cmp #$c0
    bne exact
    dey
    lda (ptr),y
    dey
    ora (ptr),y
    bne exact
    lda rc5
    and #$7f
    cmp #$7f
    bne bad
    lda rc4
    asl a
    ora rc3
    ora rc2
    bne okrec
    beq bad
.endif
exact:
    ldy #8
    lda (ptr),y
    cmp rc2
    bne bad
    iny
    lda (ptr),y
    cmp rc3
    bne bad
    iny
    lda (ptr),y
    cmp rc4
    bne bad
    iny
    lda (ptr),y
    cmp rc5
    beq okrec
bad:
    inc fail
okrec:
    clc
    lda ptr
    adc #12
    sta ptr
    bcc :+
    inc ptr+1
:   lda cnt
    bne :+
    dec cnt+1
:   dec cnt
    lda cnt
    ora cnt+1
    bne loop
    lda fail
    jmp $FFF9
