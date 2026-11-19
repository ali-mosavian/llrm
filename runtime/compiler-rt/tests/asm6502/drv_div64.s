; ca65 driver for __sf_div64: records ma (8), mb (8), q (8), r (8), little endian. Exit code: failing records (255 at most).
; A record also fails if rc0, rc1 or the callee-saved rc8..rc15 change.
.export _main
.import __sf_div64, sf_d64_ma, sf_d64_mb, sf_d64_q, sf_d64_r
.include "vec.inc"
rc8 = $88
ptr = $a0
cntr = $a2
fail = $a4
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
    ldy #7
:   lda (ptr),y
    sta sf_d64_ma,y
    dey
    bpl :-
    ldy #15
:   lda (ptr),y
    sta sf_d64_mb-8,y
    dey
    cpy #7
    bne :-
    ldx #7
:   txa
    eor #$5a
    sta rc8,x
    dex
    bpl :-
    jsr __sf_div64
    ldx #7
:   txa
    eor #$5a
    cmp rc8,x
    bne bad
    dex
    bpl :-
    ldy #16
    ldx #0
:   lda (ptr),y
    cmp sf_d64_q,x
    bne bad
    iny
    inx
    cpx #8
    bne :-
    ldx #0
:   lda (ptr),y
    cmp sf_d64_r,x
    bne bad
    iny
    inx
    cpx #8
    bne :-
    beq next
bad:
    lda fail
    cmp #255
    beq next
    inc fail
next:
    clc
    lda ptr
    adc #32
    sta ptr
    bcc :+
    inc ptr+1
:   lda cntr
    bne :+
    dec cntr+1
:   dec cntr
    lda cntr
    ora cntr+1
    beq :+
    jmp loop
:   lda fail
    ldx #0
    rts
