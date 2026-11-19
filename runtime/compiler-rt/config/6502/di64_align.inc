; The divide's entry: divide by zero, N < d, then d aligned to N's bit length and the step count in cnt. Falls into the step loop.
udm:
    lda #0
    sta dz
    lda d
    ora d+1
    ora d+2
    ora d+3
    ora d+4
    ora d+5
    ora d+6
    ora d+7
    bne dok
    ldx #7                  ; divide by zero: all ones, the remainder stays the dividend
    lda #$ff
    sta dz
:   sta qv,x
    dex
    bpl :-
    rts
dok:
    lda #0
    ldx #7
:   sta qv,x
    dex
    bpl :-
    ldx #7                  ; N < d: quotient 0, remainder N
:   lda N,x
    cmp d,x
    bne :+
    dex
    bpl :-
    sec
:   bcs align
    rts
align:                      ; s = bitlength(N) - bitlength(d); d <<= s; s + 1 steps
    ldx #7
:   lda N,x
    bne :+
    dex
    bpl :-
:   stx ln
    ldy #0
:   iny
    lsr a
    bne :-
    txa
    asl a
    asl a
    asl a
    sty cnt
    clc
    adc cnt
    sta sh                  ; bitlength(N)
    ldx #7
:   lda d,x
    bne :+
    dex
    bpl :-
:   stx ld
    ldy #0
:   iny
    lsr a
    bne :-
    txa
    asl a
    asl a
    asl a
    sty cnt
    clc
    adc cnt                 ; bitlength(d)
    eor #$ff
    sec
    adc sh
    sta sh                  ; s
    lsr a
    lsr a
    lsr a
    sta nb                  ; bytes to move d up
    beq nomove
    ldx ld
:   txa
    clc
    adc nb
    tay
    lda d,x
    sta d,y
    dex
    bpl :-
    ldx nb
    lda #0
:   dex
    sta d,x
    bne :-
nomove:
    lda sh
    and #7
    beq noshl
    tay
:   asl d
    rol d+1
    rol d+2
    rol d+3
    rol d+4
    rol d+5
    rol d+6
    rol d+7
    dey
    bne :-
noshl:
    lda sh
    clc
    adc #1
    sta cnt
