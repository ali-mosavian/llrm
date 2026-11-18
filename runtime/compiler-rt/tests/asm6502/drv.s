; ca65 driver: runs __addsf3 or __subsf3 (OP) over vec, exit code = failures (max 255); first failing record index in $c0
.export _main, vec
.include "vec.inc"
.include "op.inc"
rc0 = $80
rc2 = $82
rc8 = $88
rc9 = $89
ptr = $a0
cnt = $a2
fail = $a4
_main:
    lda #<vec
    sta ptr
    lda #>vec
    sta ptr+1
    lda #<nrec
    sta cnt
    lda #>nrec
    sta cnt+1
    lda #0
    sta fail
loop:
    lda #$5a                ; rc8 and rc9 are the caller's: they must come back unchanged
    sta rc8
    lda #$a5
    sta rc9
    ldy #7                  ; a2:a3 and b in rc2..rc7, a0:a1 in A:X
:   lda (ptr),y
    sta rc2-2,y
    dey
    cpy #1
    bne :-
    lda (ptr),y
    tax
    dey
    lda (ptr),y
    jsr OP
    sta $d0
    stx $d1
    ; compare with expected (bytes 8..11)
    ldy #8
    lda (ptr),y
    sta $e0
    iny
    lda (ptr),y
    sta $e1
    iny
    lda (ptr),y
    sta $e2
    iny
    lda (ptr),y
    sta $e3
    lda rc2
    sta $d2
    lda rc2+1
    sta $d3
    lda rc8
    cmp #$5a
    bne bad
    lda rc9
    cmp #$a5
    bne bad
    ; NaN expectation: any NaN passes
.ifdef EXACT
    jmp exact
.endif
    lda $e3
    cmp #$7f
    bne exact
    lda $e2
    cmp #$c0
    bne exact
    lda $e1
    ora $e0
    bne exact
    lda $d3
    and #$7f
    cmp #$7f
    bne bad
    lda $d2
    asl a
    ora $d1
    ora $d0
    bne okrec
    beq bad
exact:
    lda $d0
    cmp $e0
    bne bad
    lda $d1
    cmp $e1
    bne bad
    lda $d2
    cmp $e2
    bne bad
    lda $d3
    cmp $e3
    beq okrec
bad:
.ifdef SEL
    ldx #SEL
    cpx #2
    bcs @g
    lda ptr,x
    ldx #0
    rts
@g: lda $d0-2,x
    ldx #0
    rts
.endif
    inc fail
    lda fail
    cmp #1
    bne okrec
    lda ptr
    sta $c0
    lda ptr+1
    sta $c1
    ldx #11
:   lda $d0,x
    ldy #0
    dex
    bpl :-
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
    beq done
    jmp loop
done:
    lda fail
    ldx #0
    rts
