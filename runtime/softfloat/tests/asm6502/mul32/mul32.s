; 32 x 32 -> 32 multiply cores for the 6502, three ways, measured by bench.sh. VAR: 1 shift-add, 2 quarter-square 8x8
; table (1 KB), 3 nibble-product 4x4 table (256 B). a at a0..a3, b at b0..b3 (low first), result r0..r3.
.setcpu "6502"
.export mul32
a0=$b0
a1=$b1
a2=$b2
a3=$b3
b0=$b4
b1=$b5
b2=$b6
b3=$b7
r0=$b8
r1=$b9
r2=$ba
r3=$bb
pl=$bc
ph=$bd
s0=$be
s1=$bf
t=$c0
ll=$c1
hh=$c2
n0=$c4          ; nibble forms of a: al4_i = (a_i & 15) << 4 at n0+i, ah4_i = a_i & $f0 at n0+4+i
m0=$cc          ; of b: bl_j = b_j & 15 at m0+j, bh_j = b_j >> 4 at m0+4+j
.segment "CODE"
.if VAR = 1
mul32:
    lda #0
    sta r0
    sta r1
    sta r2
    sta r3
loop:
    lsr b3
    ror b2
    ror b1
    ror b0
    bcc ns
    clc
    lda r0
    adc a0
    sta r0
    lda r1
    adc a1
    sta r1
    lda r2
    adc a2
    sta r2
    lda r3
    adc a3
    sta r3
ns: asl a0
    rol a1
    rol a2
    rol a3
    lda b0
    ora b1
    ora b2
    ora b3
    bne loop
    rts
.endif
.if VAR = 2
; pl:ph = x * y = sq(x+y) - sq(|x-y|), sq(n) = n*n/4 rounded down, tables of 512 entries in two pages each
.macro MUL8 xa, yb
.local big, dif
    lda xa
    clc
    adc yb
    bcs big
    tax
    lda sqlo,x
    sta s0
    lda sqhi,x
    sta s1
    jmp dif
big:
    tax
    lda sqlo+256,x
    sta s0
    lda sqhi+256,x
    sta s1
dif:
    lda xa
    sec
    sbc yb
    bcs :+
    eor #$ff
    adc #1
:   tay
    lda s0
    sec
    sbc sqlo,y
    sta pl
    lda s1
    sbc sqhi,y
    sta ph
.endmacro
.macro ADDP k
.if k = 3
    clc
    lda r3
    adc pl
    sta r3
.else
    clc
    lda r0+k
    adc pl
    sta r0+k
    lda r1+k
    adc ph
    sta r1+k
.if k < 2
    bcc :+
    inc r2+k
.if k = 0
    bne :+
    inc r3
.endif
:
.endif
.endif
.endmacro
mul32:
    MUL8 a0, b0
    lda pl
    sta r0
    lda ph
    sta r1
    lda #0
    sta r2
    sta r3
    MUL8 a0, b1
    ADDP 1
    MUL8 a1, b0
    ADDP 1
    MUL8 a0, b2
    ADDP 2
    MUL8 a1, b1
    ADDP 2
    MUL8 a2, b0
    ADDP 2
    MUL8 a0, b3
    ADDP 3
    MUL8 a1, b2
    ADDP 3
    MUL8 a2, b1
    ADDP 3
    MUL8 a3, b0
    ADDP 3
    rts
.segment "RODATA"
sqlo:
.repeat 512, n
    .byte <((n * n) / 4)
.endrepeat
sqhi:
.repeat 512, n
    .byte >((n * n) / 4)
.endrepeat
.endif
.if VAR = 3
.macro SPLIT i
    lda a0+i
    and #$f0
    sta n0+4+i
    lda a0+i
    asl a
    asl a
    asl a
    asl a
    sta n0+i
    lda b0+i
    and #$0f
    sta m0+i
    lda b0+i
    lsr a
    lsr a
    lsr a
    lsr a
    sta m0+4+i
.endmacro
.macro MUL8 i, j, low
    lda n0+i                ; xl * yl
    ora m0+j
    tax
    lda nt,x
    sta ll
    lda n0+i                ; xl * yh
    ora m0+4+j
    tax
    lda nt,x
    sta t
    lda n0+4+i              ; xh * yl
    ora m0+j
    tax
    lda nt,x
.if low
    clc
    adc t
    asl a
    asl a
    asl a
    asl a
    clc
    adc ll
    sta pl
.else
    clc
    adc t
    sta t
    ror a                   ; the ninth bit of the middle sum into bit 7
    lsr a
    lsr a
    lsr a
    sta ph
    lda t
    asl a
    asl a
    asl a
    asl a
    clc
    adc ll
    sta pl
    lda n0+4+i              ; xh * yh
    ora m0+4+j
    tax
    lda nt,x
    adc ph
    sta ph
.endif
.endmacro
.macro ADDP k
.if k = 3
    clc
    lda r3
    adc pl
    sta r3
.else
    clc
    lda r0+k
    adc pl
    sta r0+k
    lda r1+k
    adc ph
    sta r1+k
.if k < 2
    bcc :+
    inc r2+k
.if k = 0
    bne :+
    inc r3
.endif
:
.endif
.endif
.endmacro
mul32:
    SPLIT 0
    SPLIT 1
    SPLIT 2
    SPLIT 3
    MUL8 0, 0, 0
    lda pl
    sta r0
    lda ph
    sta r1
    lda #0
    sta r2
    sta r3
    MUL8 0, 1, 0
    ADDP 1
    MUL8 1, 0, 0
    ADDP 1
    MUL8 0, 2, 0
    ADDP 2
    MUL8 1, 1, 0
    ADDP 2
    MUL8 2, 0, 0
    ADDP 2
    MUL8 0, 3, 1
    ADDP 3
    MUL8 1, 2, 1
    ADDP 3
    MUL8 2, 1, 1
    ADDP 3
    MUL8 3, 0, 1
    ADDP 3
    rts
.segment "RODATA"
nt:
.repeat 16, i
.repeat 16, j
    .byte i * j
.endrepeat
.endrepeat
.endif
