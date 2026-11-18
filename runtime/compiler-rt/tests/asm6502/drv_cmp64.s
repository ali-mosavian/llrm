; ca65 driver for config/6502/cmp64.s: records a (8), b (8), eq/le result (1), ge result (1). Exit code: failing records (255 at most).
; A record also fails if rc0, rc1 or the callee-saved rc8..rc15 change, or if X is not the sign extension of A.
.export _main
.import __ledf2, __gedf2
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
stk:    .res 8
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
    ldy #8                  ; b0..b7 on the stack
:   lda (ptr),y
    sta stk-8,y
    iny
    cpy #16
    bne :-
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
    jsr load
    jsr __ledf2
    ldy #16
    jsr check
    jsr load
    jsr __gedf2
    ldy #17
    jsr check
    clc
    lda ptr
    adc #18
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
load:                       ; a0:a1 in A:X, a2..a7 in rc2..rc7
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
