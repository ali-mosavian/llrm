; ca65 driver for config/6502/cmp32.s: records a (4), b (4), eq/le result (1), ge result (1). Exit code: failing records (255 at most).
; A record also fails if rc0, rc1 or the callee-saved rc8..rc15 change, or if X is not the sign extension of A.
.export _main
.import __lesf2, __gesf2
.include "vec.inc"
rc0 = $80
rc1 = $81
rc2 = $82
rc8 = $88
ptr = $a0
cntr = $a2
fail = $a4
xwant = $a5
.bss
stk:    .res 2
.code
_main:
    lda #<vec
    sta ptr
    lda #>vec
    sta ptr+1
    lda #<nrec
    sta cntr
    lda #>nrec
    sta cntr+1
    lda #0
    sta fail
loop:
    ldy #5                  ; a0..a3 in rc2..rc5, b0:b1 in rc6:rc7, b2:b3 on the stack
    jsr load
    ldy #6
    lda (ptr),y
    sta stk
    iny
    lda (ptr),y
    sta stk+1
    lda #<stk
    sta rc0
    lda #>stk
    sta rc1
    ldx #7
:   txa
    eor #$5a
    sta rc8,x
    dex
    bpl :-
    jsr __lesf2
    ldy #8
    jsr check
    jsr load
    jsr __gesf2
    ldy #9
    jsr check
    clc
    lda ptr
    adc #10
    sta ptr
    bcc :+
    inc ptr+1
:   lda cntr
    bne :+
    dec cntr+1
:   dec cntr
    lda cntr
    ora cntr+1
    bne loop
    lda fail
    rts
load:                       ; a, b0:b1 into rc2..rc7
    ldy #5
:   lda (ptr),y
    sta rc2,y
    dey
    bpl :-
    rts
check:                      ; A:X against the record byte at (ptr),y
    cmp (ptr),y
    bne bad
    lda (ptr),y             ; X is the sign extension of A
    and #$80
    beq :+
    lda #$FF
:   sta xwant
    cpx xwant
    bne bad
    ldx #7
:   txa
    eor #$5a
    cmp rc8,x
    bne bad
    dex
    bpl :-
    rts
bad:
    lda fail
    cmp #255
    beq :+
    inc fail
:   rts
