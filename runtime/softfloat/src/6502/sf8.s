; 8-bit float (OCP FP8 E5M2: sign, 5 exponent bits bias 15, 2 mantissa bits; denormals, inf, NaN; round to nearest even)
; add, sub, mul, div for the 6502, hand written. Args rc2 and rc3, result A, scratch rc2..rc7 and rc16..rc25.
;
; The tricks: the low seven bits are a monotonic code, so |a| < |b| is an integer compare and rounding up is "code + 1",
; which carries from the mantissa into the exponent and from the largest finite value into infinity by itself;
; every operation ends in one packer that works on a left-aligned 8-bit significand with a sticky byte.

.setcpu "6502"
rc2 = $82
rc3 = $83
rc4 = $84
rc5 = $85
rc6 = $86
rc7 = $87
rc16 = $90
rc17 = $91
rc18 = $92
rc19 = $93
rc20 = $94
rc21 = $95
rc22 = $96
rc23 = $97
rc24 = $98
rc25 = $99
.export __fp8_add, __fp8_sub, __fp8_mul, __fp8_div

sa  = rc16              ; operand a: sign ($80/0), biased exponent (denormal: 1), significand (implicit one in bit 2)
ea  = rc17
ma  = rc18
sb  = rc19
eb  = rc20
mb  = rc21
bm  = rc22              ; b aligned
cd  = rc23              ; code under construction
rsg = rc4               ; packer inputs: sign, sticky, exponent of the leading one (signed), significand left aligned
st  = rc5
pe  = rc6
pm  = rc7
t1  = rc24
t2  = rc25

.segment "CODE"

__fp8_sub:
    lda rc3
    eor #$80
    sta rc3
__fp8_add:
    lda rc2                 ; order by magnitude: the code order is the value order
    and #$7f
    sta t1
    lda rc3
    and #$7f
    cmp t1
    bcc :+
    beq :+
    ldx rc2
    lda rc3
    sta rc2
    stx rc3
:   jsr unpab
    lda ea
    cmp #31
    bne *+5
    jmp addsp
    lda eb
    cmp #31
    bne *+5
    jmp addsp
    lda sa
    sta rsg
    lda ea
    sta pe
    sec
    sbc eb
    tay                     ; exponent difference
    lda ma
    asl a
    asl a
    asl a
    asl a
    asl a
    sta pm
    lda mb
    asl a
    asl a
    asl a
    asl a
    asl a
    sta bm
    lda #0
    sta st
    tya
    beq aligned
    cpy #9
    bcc :+
    ldy #8
:   lsr bm
    bcc :+
    lda #1
    sta st
:   dey
    bne :--
aligned:
    lda sa
    eor sb
    bmi dosub
    lda pm
    clc
    adc bm
    sta pm
    bcc pk
    ror pm                  ; carry out: shift down, the lost bit is sticky
    bcc :+
    lda #1
    sta st
:   inc pe
    jmp pk
dosub:
    lda pm
    sec
    sbc bm
    sta pm
    lda st                  ; bits below bm make the subtrahend a little larger: borrow one, keep them sticky
    beq :+
    dec pm
:   lda pm
    bne nloop
    lda #0                  ; exact cancellation gives +0
    rts
nloop:
    lda pm
    bmi pk
    lda pe
    cmp #2
    bcc pk
    dec pe
    asl pm
    bne nloop               ; pm is nonzero, the loop ends at the top bit or at exponent 1

; ---- pack: sign rsg, sticky st, leading-one exponent pe (signed, may be <= 0), significand pm with its leading one in bit 7
pk:
    lda pe
    bmi pden
    beq pden
    cmp #31
    bcs pinf
pnorm:
    sec
    sbc #1
    asl a
    asl a
    sta cd
    lda pm
    lsr a
    lsr a
    lsr a
    lsr a
    lsr a
    clc
    adc cd
    sta cd                  ; (e-1)*4 + top three bits
    lda pm
    and #$10                ; guard bit
    beq pdone
    lda pm
    and #$0f
    ora st
    bne pup
    lda cd
    and #1
    beq pdone
pup:
    inc cd
pdone:
    lda cd
    cmp #$7c
    bcc :+
    lda #$7c
:   ora rsg
    rts
pinf:
    lda #$7c
    ora rsg
    rts
pden:                       ; below the normal range: shift into the denormal position, 1 - pe places
    lda #1
    sec
    sbc pe
    tay
    cpy #9
    bcc :+
    ldy #8
:   lsr pm
    bcc :+
    lda #1
    sta st
:   dey
    bne :--
    lda #1
    sta pe
    jmp pnorm

; ---- multiply
__fp8_mul:
    jsr unpab
    lda ea
    cmp #31
    bne *+5
    jmp mulsp
    lda eb
    cmp #31
    bne *+5
    jmp mulsp
    lda sa
    eor sb
    sta rsg
    lda ma
    beq zret
    lda mb
    beq zret
    jsr nrma
    jsr nrmb
    lda #0                  ; P = ma * mb, 16..49
    ldx #3
:   lsr mb
    bcc :+
    clc
    adc ma
:   asl ma
    dex
    bne :--
    sta pm
    lda ea
    clc
    adc eb
    sec
    sbc #15
    sta pe
    lda #0
    sta st
    lda pm
    cmp #32
    bcs :+
    asl pm                  ; leading one at bit 4: three places up, exponent as is
    asl pm
    asl pm
    jmp pk
:   asl pm                  ; leading one at bit 5: two places up, exponent one higher
    asl pm
    inc pe
    jmp pk
zret:
    lda rsg
    rts

; ---- divide
__fp8_div:
    jsr unpab
    lda ea
    cmp #31
    bne *+5
    jmp divsp
    lda eb
    cmp #31
    bne *+5
    jmp divsp
    lda sa
    eor sb
    sta rsg
    lda mb
    bne :+
    lda ma                  ; x / 0: infinity, 0 / 0: NaN
    beq retnan
    lda #$7c
    ora rsg
    rts
:   lda ma
    beq zret
    jsr nrma
    jsr nrmb
    lda ma                  ; q = floor(16 ma / mb), five restoring steps
    ldy #5
    ldx #0
    stx t1
:   cmp mb
    bcc :+
    sbc mb
:   rol t1
    asl a
    dey
    bne :--
    ldx #0
    cmp #0
    beq :+
    inx
:   stx st                  ; sticky: a nonzero remainder
    lda ea
    sec
    sbc eb
    clc
    adc #14
    sta pe
    lda t1
    cmp #16
    bcs :+
    asl a                   ; ratio below one: leading one at bit 3, four places up
    asl a
    asl a
    asl a
    sta pm
    jmp pk
:   asl a                   ; ratio at least one: leading one at bit 4, three places up, exponent one higher
    asl a
    asl a
    sta pm
    inc pe
    jmp pk

; ---- infinities and NaNs
cls:                        ; class of the operand at X (0 a, 3 b): 0 finite, 1 inf, 2 NaN
    lda rc17,x
    cmp #31
    bne cfin
    lda rc18,x
    cmp #4
    beq cinf
    lda #2
    rts
cinf:
    lda #1
    rts
cfin:
    lda #0
    rts
retnan:
    lda #$7e
    rts
spnan:                      ; NaN operand: drop spcls's return address, answer for its caller
    pla
    pla
    lda #$7e
    rts
spcls:
    ldx #0
    jsr cls
    sta t1
    ldx #3
    jsr cls
    sta t2
    cmp #2
    beq spnan
    lda t1
    cmp #2
    beq spnan
    lda sa
    eor sb
    sta rsg
    rts
addsp:
    jsr spcls
    lda t1
    beq addb
    lda t2
    beq adda
    lda sa
    cmp sb
    bne retnan
adda:
    lda sa
    ora #$7c
    rts
addb:
    lda sb
    ora #$7c
    rts
mulsp:
    jsr spcls
    lda t1
    beq mulbi
    lda t2                  ; a is infinite: times a zero is NaN
    bne mulinf
    lda mb
    beq retnan
mulinf:
    lda rsg
    ora #$7c
    rts
mulbi:                      ; b is infinite, a finite
    lda ma
    beq retnan
    bne mulinf
divsp:
    jsr spcls
    lda t1
    beq divbi
    lda t2
    bne retnan              ; inf / inf
    beq mulinf
divbi:                      ; finite / inf = 0
    lda rsg
    rts

; ---- operand unpack: rc2, rc3 into sa/ea/ma and sb/eb/mb
unpab:
    ldy #0
    ldx #0
    jsr unp
    ldy #1
    ldx #3
unp:                        ; byte rc2+Y into the three cells at rc16+X
    lda rc2,y
    sta t1
    and #$80
    sta rc16,x
    lda t1
    lsr a
    lsr a
    and #$1f
    bne :+
    lda #1                  ; denormal: exponent 1, no implicit one
    sta rc17,x
    lda t1
    and #3
    sta rc18,x
    rts
:   sta rc17,x
    lda t1
    and #3
    ora #4
    sta rc18,x
    rts
nrma:                       ; normalize a nonzero denormal significand, lowering the exponent
    ldx #0
    beq nrm
nrmb:
    ldx #3
nrm:
    lda rc18,x
:   cmp #4
    bcs :+
    asl a
    dec rc17,x
    jmp :-
:   sta rc18,x
    rts
