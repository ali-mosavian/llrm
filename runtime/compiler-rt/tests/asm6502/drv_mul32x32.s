; ca65 driver for __mul32x32: records a (4), b (4), a*b (8), little endian. Exit code: failing records (255 at most).
; A record also fails if rc0, rc1 or the callee-saved rc8..rc15 change.
.export _main
.import __mul32x32
.include "vec.inc"
rc0 = $80
rc2 = $82
rc6 = $86
rc8 = $88
ptr = $a0
cntr = $a2
fail = $a4
.bss
stk: .res 6
lo: .res 4
hi: .res 4
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
    ldy #5                  ; a, b0, b1
:   lda (ptr),y
    sta rc2,y
    dey
    bpl :-
    ldy #6
    lda (ptr),y
    sta stk
    iny
    lda (ptr),y
    sta stk+1
    lda #<hi
    sta stk+2
    lda #>hi
    sta stk+3
    lda #<lo
    sta stk+4
    lda #>lo
    sta stk+5
    lda #<stk
    sta rc0
    lda #>stk
    sta rc0+1
    ldx #7
:   txa
    eor #$5a
    sta rc8,x
    dex
    bpl :-
    jsr __mul32x32
    ldx #7
:   txa
    eor #$5a
    cmp rc8,x
    bne bad
    dex
    bpl :-
    lda rc0
    cmp #<stk
    bne bad
    lda rc0+1
    cmp #>stk
    bne bad
    ldy #8
    ldx #0
:   lda (ptr),y
    cmp lo,x
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
    adc #16
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
