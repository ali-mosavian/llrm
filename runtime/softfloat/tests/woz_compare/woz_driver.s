; Cycle/dump driver for the Rankin/Woz routines. -D OP=0..5 (add sub mul div float fix), -D NOCALL (loop only), -D DUMP.
.export _main
.import pushax, _write
.import __WOZA_LOAD__, __WOZA_RUN__, __WOZA_SIZE__, __WOZB_LOAD__, __WOZB_RUN__, __WOZB_SIZE__
.include "woz_equ.inc"
ptr = $20
cnt = $22
outp = $23

.segment "BSS"
out: .res 4 * NREC
.segment "RODATA"
tab:
.include "woz_tab.inc"
.segment "CODE"
.macro move_code load, run, size
    lda #<load
    sta $24
    lda #>load
    sta $25
    lda #<run
    sta $26
    lda #>run
    sta $27
    ldy #<(size-1)
:   lda ($24),y
    sta ($26),y
    dey
    cpy #$FF
    bne :-
.endmacro
_main:
    move_code __WOZA_LOAD__, __WOZA_RUN__, __WOZA_SIZE__
    move_code __WOZB_LOAD__, __WOZB_RUN__, __WOZB_SIZE__
    lda #<tab
    sta ptr
    lda #>tab
    sta ptr+1
    lda #<out
    sta outp
    lda #>out
    sta outp+1
    lda #NREC
    sta cnt
loop:
    ldy #7
cp: lda (ptr),y
    sta fpregs,y
    dey
    bpl cp
.ifndef NOCALL
    jsr target
.endif
.ifdef DUMP
    ldy #3
dp: lda X1,y
    sta (outp),y
    dey
    bpl dp
    clc
    lda outp
    adc #4
    sta outp
    bcc :+
    inc outp+1
:
.endif
    clc
    lda ptr
    adc #8
    sta ptr
    bcc :+
    inc ptr+1
:   dec cnt
    bne loop
.ifdef DUMP
    lda #1
    ldx #0
    jsr pushax
    lda #<out
    ldx #>out
    jsr pushax
    lda #<(4 * NREC)
    ldx #>(4 * NREC)
    jsr _write
.endif
    lda #0
    ldx #0
    rts
