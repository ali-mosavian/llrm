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
stk: .res 4
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
    lda #<hi                ; hi and lo, the pointers on the stack
    sta stk
    lda #>hi
    sta stk+1
    lda #<lo
    sta stk+2
    lda #>lo
    sta stk+3
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
    ldy #7                  ; a2:a3 in rc2:rc3, b in rc4..rc7, a0:a1 in A:X
:   lda (ptr),y
    sta rc2-2,y
    dey
    cpy #1
    bne :-
    lda (ptr),y
    tax
    dey
    lda (ptr),y
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
