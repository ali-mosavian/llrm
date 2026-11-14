; 64-bit shifts for the 6502: u64 __ashldi3(u64 a, int n), __lshrdi3, i64 __ashrdi3 (libgcc signatures).
; ABI (llrm-target-6502): a as words low first, w0 in A:X, w1 in rc2:rc3, w2 in rc4:rc5, w3 in rc6:rc7, the 16-bit count n
; on the caller's stack at (rc0:rc1)+0..1, read only here. Result A:X:rc2..rc7, low byte first.
; A count of 64 or more (as unsigned) gives 0, or the sign for the arithmetic shift. Scratch rc16..rc24.

.setcpu "6502"
.export __ashldi3, __lshrdi3, __ashrdi3

rc0 = $80
rc2 = $82
rc16 = $90
rc23 = $97
cnt = $98               ; rc24

.segment "CODE"

load:                   ; a into rc16..rc23, count into cnt; carry set when the count is 64 or more
    sta rc16
    stx rc16+1
    ldx #5
:   lda rc2,x
    sta rc16+2,x
    dex
    bpl :-
    ldy #0
    lda (rc0),y
    sta cnt
    iny
    lda (rc0),y
    bne big
    lda cnt
    cmp #64
    rts
big:
    sec
    rts

done:                   ; result from rc16..rc23
    ldx #5
:   lda rc16+2,x
    sta rc2,x
    dex
    bpl :-
    ldx rc16+1
    lda rc16
    rts

fill:                   ; all eight bytes A
    ldx #7
:   sta rc16,x
    dex
    bpl :-
    jmp done

__ashldi3:
    jsr load
    bcc la
    lda #0
    jmp fill
la: lda cnt
lb: cmp #8
    bcc lc
    sbc #8                  ; one whole byte up
    pha
    ldx #7
:   lda rc16-1,x
    sta rc16,x
    dex
    bne :-
    stx rc16
    pla
    jmp lb
lc: tay
    beq ld
:   asl rc16
    rol rc16+1
    rol rc16+2
    rol rc16+3
    rol rc16+4
    rol rc16+5
    rol rc16+6
    rol rc23
    dey
    bne :-
ld: jmp done

__lshrdi3:
    jsr load
    bcc ra
    lda #0
    jmp fill
ra: lda cnt
rb: cmp #8
    bcc rc
    sbc #8                  ; one whole byte down
    pha
    ldx #0
:   lda rc16+1,x
    sta rc16,x
    inx
    cpx #7
    bne :-
    lda #0
    sta rc23
    pla
    jmp rb
rc: tay
    beq rd
:   lsr rc23
    ror rc16+6
    ror rc16+5
    ror rc16+4
    ror rc16+3
    ror rc16+2
    ror rc16+1
    ror rc16
    dey
    bne :-
rd: jmp done

__ashrdi3:
    jsr load
    bcc sa
    lda rc23
    and #$80
    beq sz
    lda #$ff
sz: jmp fill
sa: lda cnt
sb: cmp #8
    bcc sc
    sbc #8                  ; one whole byte down, the sign fills the top
    pha
    ldx #0
:   lda rc16+1,x
    sta rc16,x
    inx
    cpx #7
    bne :-
    lda rc23
    and #$80
    beq :+
    lda #$ff
:   sta rc23
    pla
    jmp sb
sc: tay
    beq sd
:   lda rc23
    cmp #$80                ; carry = sign
    ror rc23
    ror rc16+6
    ror rc16+5
    ror rc16+4
    ror rc16+3
    ror rc16+2
    ror rc16+1
    ror rc16
    dey
    bne :-
sd: jmp done
