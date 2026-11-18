; ca65 driver for config/6502/cmp32.s: records a (4), b (4), eq/le result (1), ge result (1). Exit code: failing records (255 at most).
; A record also fails if the callee-saved rc8..rc15 change, or if X is not the sign extension of A.
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
    ldx #7                  ; the callee-saved cells get a pattern the call must leave alone
:   txa
    eor #$5a
    sta rc8,x
    dex
    bpl :-
    jsr load
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
load:                       ; a0:a1 in A:X, a2:a3 and b in rc2..rc7
    ldy #7
:   lda (ptr),y
    sta rc2-2,y
    dey
    cpy #1
    bne :-
    lda (ptr),y
    tax
    dey
    lda (ptr),y
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
