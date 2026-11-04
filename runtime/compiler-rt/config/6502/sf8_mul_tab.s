; FP8 multiply by table lookup: __fp8_mul_t(a, b) with the same ABI as sf8.s (rc2, rc3, result A).
; The table is the exact magnitude code of |a| * |b| for every pair (inf and NaN included, 16 KB), made by
; tools/gen_fp8_mul_table.py into fp8_mul_tab.inc (-I path); the sign is the xor of the signs.

.setcpu "6502"
.export __fp8_mul_t

rc2 = $82
rc3 = $83
ptr = $90               ; rc16:rc17
tmp = $92               ; rc18

.segment "CODE"

__fp8_mul_t:
    lda rc2                 ; ptr = tab + |a| * 128
    and #$7f
    lsr a
    sta ptr+1
    lda #0
    ror a
    clc
    adc #<tab
    sta ptr
    lda ptr+1
    adc #>tab
    sta ptr+1
    lda rc3
    and #$7f
    tay
    lda (ptr),y
    sta tmp
    lda rc2
    eor rc3
    and #$80
    ora tmp
    rts

.segment "RODATA"
tab:
.include "fp8_mul_tab.inc"
