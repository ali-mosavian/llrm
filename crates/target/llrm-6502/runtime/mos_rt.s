; Spike runtime for the 6502 target: what the compiler would call for multiply, divide, remainder and
; shifts by a variable. The `mos` convention: a byte argument in the first free of rc2.., a word in
; rs1 (rc2:rc3) then rs2 (rc4:rc5), a byte result in A, a word in A (low):X (high). The routines may
; change A, X, Y and rc2..rc7 and keep rc0, rc1 and rc8..rc15. The caller defines rc0..rc(N-1), N being
; the platform row's `zero_page` (16 to 26 bytes from its zero-page base).
;
; Zero-page registers: rc0:rc1 is the soft stack pointer; rc2..rc7 are arguments and caller-saved
; scratch; rc8..rc15 are callee-saved (a function that uses one saves it on entry); rc16..rc(N-1) are
; caller-saved scratch like rc2..rc7, which a compiled function may change and a call may clobber,
; and which these routines never touch. N is per platform because the zero page is: C64, Apple II,
; NES, Atari 8-bit, BBC and sim65 all leave 26 bytes (cc65 keeps about the same for sreg, tmp1..4 and
; ptr1..4), so N = 26; a platform with less lists less. The compiler uses the scratch cells before
; the callee-saved ones: they cost no save and restore.
;
; Division by zero gives the all-ones quotient and the dividend as remainder (the restoring loop
; below, unchanged). Signed division truncates toward zero and the remainder has the dividend's sign.

; ---- callee-saved cells on the hardware stack, for a function that keeps three or more -------------

; __csave: Y = n; pushes rc8..rc(7+n) in that order under the caller's return address, which it
; holds in __ra meanwhile. Changes A, X, Y. __crest: X = n, A (a byte result) in Y; the function's
; own return: pops rc(7+n)..rc8 and returns to the function's caller. Changes A, X.
.export __csave
__csave:
  pla
  sta __ra
  pla
  sta __ra+1
  ldx #1
@next:
  lda rc7,x                     ; [rc8..rc15]
  pha
  inx
  dey
  bne @next
  lda __ra+1
  pha
  lda __ra
  pha
  rts

.export __crest
__crest:
@next:
  pla
  sta rc7,x                     ; [rc8..rc15]
  dex
  bne @next
  tya
  rts

; ---- multiply: the low half of the product -------------------------------------------------------

.export __mulqi3
__mulqi3:                       ; rc2 * rc3 -> A
  lda #0
  beq @test
@add:
  clc
  adc rc2
@shift:
  asl rc2
@test:
  lsr rc3                       ; carry: the bit; zero: no multiplier bits are left
  bcs @add
  bne @shift
  rts

.export __umulqihi3
__umulqihi3:                    ; rc2 * rc3 -> A:X, both unsigned bytes
  lda #0
  ldx #8
  lsr rc3
@next:
  bcc @skip
  clc
  adc rc2
@skip:
  ror a
  ror rc3
  dex
  bne @next
  tax
  lda rc3
  rts

; two signed bytes: the unsigned product, then the high byte less the other byte for each negative one
.export __mulqihi3
__mulqihi3:                     ; rc2 * rc3 -> A:X, both signed bytes
  lda rc3
  sta rc5
  jsr __umulqihi3
  sta rc4
  txa
  ldx rc2
  bpl @a
  sec
  sbc rc5
@a:
  ldx rc5
  bpl @b
  sec
  sbc rc2
@b:
  tax
  lda rc4
  rts

.export __mulhi3
__mulhi3:                       ; rs1 * rs2 -> A:X
  lda #0
  sta rc6
  sta rc7
@next:
  lsr rc5
  ror rc4
  bcc @skip
  clc
  lda rc6
  adc rc2
  sta rc6
  lda rc7
  adc rc3
  sta rc7
@skip:
  asl rc2
  rol rc3
  lda rc4                       ; stop when no multiplier bits are left
  ora rc5
  bne @next
  lda rc6
  ldx rc7
  rts

; ---- unsigned divide ---------------------------------------------------------------------------

; rc2 / rc3: the quotient in rc2, the remainder in A. A carry out of the shifted remainder means it
; is at least the divisor.
udivmod8:
  lda #0
  ldx #8
@next:
  asl rc2
  rol a
  bcs @sub
  cmp rc3
  bcc @keep
@sub:
  sbc rc3
  inc rc2
@keep:
  dex
  bne @next
  rts

; rs1 / rs2: the quotient in rc2:rc3, the remainder in rc6:rc7. Uses A and X; rc4:rc5 is kept.
; A divisor of 256 or more makes a quotient under 256, and most are a few bits (Euclid's steps, a ratio near 1):
; the divisor shifts up to the dividend, then one restoring step a place, so the clocks follow the quotient's
; width and not 16 (compiler-rt's `__udivmodhi4`). A byte divisor under 32 times the dividend's high byte (so a
; zero divisor too) has a wide quotient: it takes the walks below, the bit loop for 256 or more and the
; short-remainder loop for a byte.
udivmod16:
  lda rc5
  bne @shift
  lda rc3
  cmp #32
  bcs @walk
  asl a
  asl a
  asl a
  cmp rc4
  bcs @walk
@shift:
  ldx #1                        ; X = the steps to take: the places the divisor shifts, and one more
@align:
  lda rc4
  cmp rc2
  lda rc5
  sbc rc3
  bcs @steps                    ; the divisor is not under the dividend
  lda rc5
  bmi @steps                    ; its top bit is set: no further shift fits 16 bits
  asl rc4
  rol rc5
  inx
  bne @align
@walk:
  lda rc5
  bne __udivhi3c
  ; a divisor under 256: the remainder is a byte, in A, with the carry of the shift as its ninth bit
  ldx #16
@next8:
  asl rc2
  rol rc3
  rol a
  bcs @sub8
  cmp rc4
  bcc @keep8
@sub8:
  sbc rc4
  inc rc2
@keep8:
  dex
  bne @next8
  sta rc6
  lda #0
  sta rc7
  rts
@steps:                         ; the dividend is the remainder; the quotient, under 256, builds in rc2
  lda rc2
  sta rc6
  lda rc3
  sta rc7
  lda #0
  sta rc2
  sta rc3
  beq @first
@next:
  lsr rc5
  ror rc4
@first:
  asl rc2
  lda rc6
  cmp rc4
  lda rc7
  sbc rc5
  bcc @keep
  sta rc7
  lda rc6
  sbc rc4
  sta rc6
  inc rc2
@keep:
  dex
  bne @next
  rts
; the bit loop alone, what the compact entry points run: the quotient in rc2:rc3 and A:X, the remainder in
; rc6:rc7. Uses A, X, Y. The remainder before step k is under 2^(k-1), so the shifted one never carries out of
; 16 bits, whatever the divisor.
.export __udivhi3c
__udivhi3c:
  lda #0
  sta rc6
  sta rc7
  ldx #16
@next:
  asl rc2
  rol rc3
  rol rc6
  rol rc7
  lda rc6
  cmp rc4
  lda rc7
  sbc rc5
  bcc @keep
  sta rc7
  lda rc6
  sbc rc4
  sta rc6
  inc rc2
@keep:
  dex
  bne @next
  lda rc2
  ldx rc3
  rts

.export __udivqi3
__udivqi3:                      ; rc2 / rc3 -> A
  jsr udivmod8
  lda rc2
  rts

.export __udivmodqi4
__udivmodqi4:                   ; rc2 / rc3 -> A, the remainder in rc6
  jsr udivmod8
  sta rc6
  lda rc2
  rts

.export __umodqi3
__umodqi3:                      ; rc2 % rc3 -> A
  jmp udivmod8

.export __udivhi3
__udivhi3:                      ; rs1 / rs2 -> A:X, the remainder in rc6:rc7
  jsr udivmod16
  lda rc2
  ldx rc3
  rts

.export __umodhi3
__umodhi3:                      ; rs1 % rs2 -> A:X
  jsr udivmod16
  lda rc6
  ldx rc7
  rts

; the compact variants, called where a byte is worth more than the clocks: no short-divisor path (__udivhi3c above)
.export __umodhi3c
__umodhi3c:                     ; rs1 % rs2 -> A:X
  jsr __udivhi3c
  lda rc6
  ldx rc7
  rts

; ---- signed divide: divide the magnitudes, then give the result its sign -------------------------

; A := -A
neg8:
  eor #$FF
  clc
  adc #1
  rts

; rc2 := |rc2|, rc3 := |rc3|
abs8_pair:
  lda rc2
  bpl @b
  jsr neg8
  sta rc2
@b:
  lda rc3
  bpl @done
  jsr neg8
  sta rc3
@done:
  rts

.export __divqi3
__divqi3:                       ; rc2 / rc3 -> A
  lda rc2
  eor rc3
  tay                           ; the quotient is negative when the signs differ
  jsr abs8_pair
  jsr udivmod8
  lda rc2
  cpy #0
  bpl @done
  jsr neg8
@done:
  rts

.export __modqi3
__modqi3:                       ; rc2 % rc3 -> A
  ldy rc2                       ; the remainder takes the dividend's sign
  jsr abs8_pair
  jsr udivmod8
  cpy #0
  bpl @done
  jsr neg8
@done:
  rts

; rs1 := |rs1|, rs2 := |rs2|: the pair at rc4:rc5, then the one at rc2:rc3
abs16_pair:
  ldx #2
@next:
  lda rc3,x
  bpl @skip
  sec
  lda #0
  sbc rc2,x
  sta rc2,x
  lda #0
  sbc rc3,x
  sta rc3,x
@skip:
  dex
  dex
  bpl @next
  rts

; rs := -rs for the pair at (lo, hi)
.macro neg16 lo, hi
  sec
  lda #0
  sbc lo
  sta lo
  lda #0
  sbc hi
  sta hi
.endmacro

; A:X := -(hi:lo); the carry chain runs through Y, so the result is returned as it is made
.macro negax lo, hi
  sec
  lda #0
  sbc lo
  tay
  lda #0
  sbc hi
  tax
  tya
.endmacro

; the result comes back from the divide in A:X; the sign is the N flag on the stack (the bit loop uses Y, `plp` leaves A)
.export __divhi3
__divhi3:                       ; rs1 / rs2 -> A:X
  lda rc3
  ora rc5
  bmi @signed
  jsr udivmod16                 ; two non-negative words: no signs to take and give back
  lda rc2
  ldx rc3
  rts
@signed:
  lda rc3
  eor rc5
  php
  jsr abs16_pair
  jsr udivmod16
  lda rc2
  ldx rc3
  plp
  bpl @done
  negax rc2, rc3
@done:
  rts

.export __modhi3
__modhi3:                       ; rs1 % rs2 -> A:X
  lda rc3
  ora rc5
  bmi @signed
  jsr udivmod16                 ; two non-negative words: no signs to take and give back
  lda rc6
  ldx rc7
  rts
@signed:
  lda rc3
  php
  jsr abs16_pair
  jsr udivmod16
  plp
  bpl @pos
  negax rc6, rc7
  rts
@pos:
  lda rc6
  ldx rc7
  rts

.export __divmodhi4
__divmodhi4:                    ; rs1 / rs2 -> A:X, the remainder (the dividend's sign) in rc6:rc7
  lda rc3
  ora rc5
  bmi @signed
  jmp __udivhi3
@signed:
  lda rc3
  pha                           ; the remainder's sign
  eor rc5
  pha                           ; the quotient's sign: the bit loop uses Y
  jsr abs16_pair
  jsr __udivhi3
  jmp divmod16_signs

; the compact signed variants, called at size: the same sign handling over the bit loop alone (no short-divisor path)
.export __divhi3c
__divhi3c:                       ; rs1 / rs2 -> A:X, compact
  lda rc3
  eor rc5
  php
  jsr abs16_pair
  jsr __udivhi3c
  plp
  bpl @done
  negax rc2, rc3
@done:
  rts

.export __modhi3c
__modhi3c:                       ; rs1 % rs2 -> A:X, compact
  lda rc3
  php
  jsr abs16_pair
  jsr __udivhi3c
  plp
  bpl @pos
  negax rc6, rc7
  rts
@pos:
  lda rc6
  ldx rc7
  rts

.export __divmodhi4c
__divmodhi4c:                    ; rs1 / rs2 -> A:X, the remainder in rc6:rc7, compact
  lda rc3
  pha
  eor rc5
  pha
  jsr abs16_pair
  jsr __udivhi3c
  jmp divmod16_signs

divmod16_signs:                 ; the quotient's sign on the stack, then the remainder's
  pla
  bpl @q
  neg16 rc2, rc3
@q:
  pla
  bpl @done
  neg16 rc6, rc7
@done:
  lda rc2
  ldx rc3
  rts

; ---- shifts by a variable count: rs1 or rc2 shifted by rc4 or rc3 ---------------------------------

.export __ashlqi3
__ashlqi3:                      ; rc2 << rc3 -> A
  ldx rc3
  beq @done
@next:
  asl rc2
  dex
  bne @next
@done:
  lda rc2
  rts

.export __lshrqi3
__lshrqi3:                      ; rc2 >> rc3 (logical) -> A
  ldx rc3
  beq @done
@next:
  lsr rc2
  dex
  bne @next
@done:
  lda rc2
  rts

.export __ashrqi3
__ashrqi3:                      ; rc2 >> rc3 (arithmetic) -> A
  ldx rc3
  beq @done
@next:
  lda rc2
  cmp #$80
  ror rc2
  dex
  bne @next
@done:
  lda rc2
  rts

.export __ashlhi3
__ashlhi3:                      ; rs1 << rc4 -> A:X
  ldx rc4
  beq @done
@next:
  asl rc2
  rol rc3
  dex
  bne @next
@done:
  lda rc2
  ldx rc3
  rts

; 1 << n for a count in A (0 to 15): the bit is made in a byte, which half it is in is the count's bit 3. A:X in, none of
; the rc cells touched.
.export __shl1hi3
__shl1hi3:                      ; A = n -> A:X = 1 << n
  ldy #0
  cmp #8
  bcc @low
  sbc #8                        ; carry is set: the bit is in the high byte
  iny
@low:
  tax
  lda #1
  cpx #0
  beq @made
@next:
  asl a
  dex
  bne @next
@made:
  cpy #0
  bne @high
  ldx #0
  rts
@high:
  tax
  lda #0
  rts

.export __lshrhi3
__lshrhi3:                      ; rs1 >> rc4 (logical) -> A:X
  ldx rc4
  beq @done
@next:
  lsr rc3
  ror rc2
  dex
  bne @next
@done:
  lda rc2
  ldx rc3
  rts

.export __ashrhi3
__ashrhi3:                      ; rs1 >> rc4 (arithmetic) -> A:X
  ldx rc4
  beq @done
@next:
  lda rc3
  cmp #$80
  ror rc3
  ror rc2
  dex
  bne @next
@done:
  lda rc2
  ldx rc3
  rts

; ---- 32-bit operations -------------------------------------------------------------------------
; The mos convention, as any function of two longs: the first operand in A:X:rc2:rc3 (A the low
; byte), the second in rc4..rc7 (a shift count in rc4), the result in A:X:rc2:rc3; a widening
; multiply takes its words in A:X and rc4:rc5. A divide with its remainder (`__divmodsi4`,
; `__udivmodsi4`) leaves the remainder in rc4..rc7. The routines change A, X, Y and rc2..rc7.
; Division by zero gives the all-ones quotient and the dividend.
;
; The shifts and the multiplies work in the argument cells. The divides are entries (`jsr in32`, the
; body, `jmp out32`) that copy the first operand to __sa, work there and in rc4..rc7 (`__sb` in the
; comments), and copy __sa back; the bodies are the s32_ labels.

; zero-page equates after the registers where the platform row has `scratch_zero_page` (tools/6502/mkrt.py), BSS otherwise
.segment "BSS"
__sa: .res 4
__sr: .res 4                   ; right after __sa: {__sa, __sr} is one eight-byte chain (the byte skip of the lean division)
__ra: .res 2
__st: .res 4
__sg: .res 1
.segment "CODE"

in32:                           ; A:X:rc2:rc3 -> __sa
  sta __sa
  stx __sa+1
  lda rc2
  sta __sa+2
  lda rc3
  sta __sa+3
  rts

out32r:                         ; the remainder __sr -> rc4..rc7, then the quotient as out32
  lda __sr
  sta rc4
  lda __sr+1
  sta rc5
  lda __sr+2
  sta rc6
  lda __sr+3
  sta rc7
  jmp out32                     ; no fall-through: a linker keeps routines apart
out32:                          ; __sa -> A:X:rc2:rc3
  lda __sa+2
  sta rc2
  lda __sa+3
  sta rc3
  ldx __sa+1
  lda __sa
  rts

out32_st:                       ; the product __st -> A:X:rc2:rc3
  lda __st+2
  sta rc2
  lda __st+3
  sta rc3
  ldx __st+1
  lda __st
  rts

; the entries: the glue, the body, the glue
.export __udivsi3
__udivsi3:
  jsr in32
  jsr s32_udivsi3
  jmp out32

.export __umodsi3
__umodsi3:
  jsr in32
  jsr s32_umodsi3
  jmp out32

.export __divsi3
__divsi3:
  jsr in32
  jsr s32_divsi3
  jmp out32

.export __modsi3
__modsi3:
  jsr in32
  jsr s32_modsi3
  jmp out32

.export __udivsi3_fast
__udivsi3_fast:
  jsr in32
  jsr s32_udivsi3_fast
  jmp out32

.export __udivsi3_fast_byte
__udivsi3_fast_byte:
  jsr in32
  jsr s32_udivsi3_fast_byte
  jmp out32

.export __udivsi3_fast_small
__udivsi3_fast_small:
  jsr in32
  jsr s32_udivsi3_fast_small
  jmp out32

.export __umodsi3_fast
__umodsi3_fast:
  jsr in32
  jsr s32_umodsi3_fast
  jmp out32

.export __umodsi3_fast_byte
__umodsi3_fast_byte:
  jsr in32
  jsr s32_umodsi3_fast_byte
  jmp out32

.export __umodsi3_fast_small
__umodsi3_fast_small:
  jsr in32
  jsr s32_umodsi3_fast_small
  jmp out32

.export __divsi3_fast
__divsi3_fast:
  jsr in32
  jsr s32_divsi3_fast
  jmp out32

.export __divsi3_fast_byte
__divsi3_fast_byte:
  jsr in32
  jsr s32_divsi3_fast_byte
  jmp out32

.export __divsi3_fast_small
__divsi3_fast_small:
  jsr in32
  jsr s32_divsi3_fast_small
  jmp out32

.export __modsi3_fast
__modsi3_fast:
  jsr in32
  jsr s32_modsi3_fast
  jmp out32

.export __modsi3_fast_byte
__modsi3_fast_byte:
  jsr in32
  jsr s32_modsi3_fast_byte
  jmp out32

.export __modsi3_fast_small
__modsi3_fast_small:
  jsr in32
  jsr s32_modsi3_fast_small
  jmp out32

.export __divmodsi4
__divmodsi4:
  jsr in32
  jsr s32_divmodsi4
  jmp out32r

.export __divmodsi4_fast
__divmodsi4_fast:
  jsr in32
  jsr s32_divmodsi4_fast
  jmp out32r

.export __divmodsi4_fast_byte
__divmodsi4_fast_byte:
  jsr in32
  jsr s32_divmodsi4_fast_byte
  jmp out32r

.export __divmodsi4_fast_small
__divmodsi4_fast_small:
  jsr in32
  jsr s32_divmodsi4_fast_small
  jmp out32r

.export __udivmodsi4
__udivmodsi4:
  jsr in32
  jsr s32_udivsi3
  jmp out32r

.export __udivmodsi4_fast
__udivmodsi4_fast:
  jsr in32
  jsr s32_udivsi3_fast
  jmp out32r

.export __udivmodsi4_fast_byte
__udivmodsi4_fast_byte:
  jsr in32
  jsr s32_udivsi3_fast_byte
  jmp out32r

.export __udivmodsi4_fast_small
__udivmodsi4_fast_small:
  jsr in32
  jsr s32_udivsi3_fast_small
  jmp out32r

.export __mulsi3
__mulsi3:                       ; A:X:rc2:rc3 * rc4..rc7 -> A:X:rc2:rc3 (the low 32 bits)
  sta __sa
  stx __sa+1
  lda #0
  sta __st
  sta __st+1
  sta __st+2
  sta __st+3
  ldx #32
@next:
  lsr rc7
  ror rc6
  ror rc5
  ror rc4
  bcc @skip
  clc
  lda __st
  adc __sa
  sta __st
  lda __st+1
  adc __sa+1
  sta __st+1
  lda __st+2
  adc rc2
  sta __st+2
  lda __st+3
  adc rc3
  sta __st+3
@skip:
  asl __sa
  rol __sa+1
  rol rc2
  rol rc3
  dex
  bne @next
  jmp out32_st

; __mulsi3 for speed: the multiplier is walked a byte at a time and the walk
; stops at its last set byte, 19 bytes more, a third of the clocks for a byte.
.export __mulsi3_fast
__mulsi3_fast:                       ; A:X:rc2:rc3 * rc4..rc7 -> A:X:rc2:rc3 (the low 32 bits)
  sta __sa
  stx __sa+1
  lda #0
  sta __st
  sta __st+1
  sta __st+2
  sta __st+3
  ldy #4
@byte:
  ldx #8
@next:
  lsr rc7
  ror rc6
  ror rc5
  ror rc4
  bcc @skip
  clc
  lda __st
  adc __sa
  sta __st
  lda __st+1
  adc __sa+1
  sta __st+1
  lda __st+2
  adc rc2
  sta __st+2
  lda __st+3
  adc rc3
  sta __st+3
@skip:
  asl __sa
  rol __sa+1
  rol rc2
  rol rc3
  dex
  bne @next
  lda rc4
  ora rc5
  ora rc6
  ora rc7
  beq @done
  dey
  bne @byte
@done:
  jmp out32_st

; 16 x 16 -> 32: the words A:X and rc4:rc5 in, the product in A:X:rc2:rc3. The product is built in
; __st:rc4:rc5, the multiplier shifting out of rc4:rc5 as the low half shifts in. A multiplier whose
; high byte is zero is spent after 8 bits: the product is then the 32 bits shifted down a byte.
.export __umulhisi3
__umulhisi3:
  sta __sa
  stx __sa+1
  lda #0
  sta __st
  sta __st+1
  ldy #2
@pass:
  ldx #8
@next:
  lda rc4
  lsr a
  bcc @skip
  clc
  lda __st
  adc __sa
  sta __st
  lda __st+1
  adc __sa+1
  sta __st+1
@skip:
  ror __st+1
  ror __st
  ror rc5
  ror rc4
  dex
  bne @next
  dey
  beq @long
  lda rc4
  bne @pass
  lda __st+1
  sta rc2
  lda #0
  sta rc3
  ldx __st
  lda rc5
  rts
@long:
  lda __st
  sta rc2
  lda __st+1
  sta rc3
  ldx rc5
  lda rc4
  rts

; the signed product: the unsigned one less the other operand from the high word for each
; negative operand
.export __mulhisi3
__mulhisi3:
  sta __sr
  stx __sr+1
  lda rc4
  sta __sr+2
  lda rc5
  sta __sr+3
  lda __sr
  jsr __umulhisi3
  tay
  lda __sr+1
  bpl @a
  sec
  lda rc2
  sbc __sr+2
  sta rc2
  lda rc3
  sbc __sr+3
  sta rc3
@a:
  lda __sr+3
  bpl @done
  sec
  lda rc2
  sbc __sr
  sta rc2
  lda rc3
  sbc __sr+1
  sta rc3
@done:
  tya
  rts

; the same for a multiplier under 256 (rc4, the high byte known zero at the call): the multiplicand
; shifts left into 24 bits and is added for each set bit, and the walk ends with the multiplier's last
; set bit
.export __umulhisi3_byte
__umulhisi3_byte:
  sta __sa
  stx __sa+1
  lda #0
  sta __st
  sta __st+1
  sta __st+2
  sta __sa+2
@bit:
  lsr rc4
  bcc @no
  clc
  lda __st
  adc __sa
  sta __st
  lda __st+1
  adc __sa+1
  sta __st+1
  lda __st+2
  adc __sa+2
  sta __st+2
@no:
  lda rc4
  beq @out
  asl __sa
  rol __sa+1
  rol __sa+2
  jmp @bit
@out:
  lda __st+2
  sta rc2
  lda #0
  sta rc3
  ldx __st+1
  lda __st
  rts

; the signed multiplicand by a byte: the unsigned product less the byte from the high word if the
; multiplicand is negative
.export __mulhisi3_byte
__mulhisi3_byte:
  stx __sr+1
  ldy rc4
  sty __sr
  jsr __umulhisi3_byte
  tay
  lda __sr+1
  bpl @done
  sec
  lda rc2
  sbc __sr
  sta rc2
  lda rc3
  sbc #0
  sta rc3
@done:
  tya
  rts

.macro cmp_sub_32
  sec
  lda __sr
  sbc rc4
  sta __st
  lda __sr+1
  sbc rc5
  sta __st+1
  lda __sr+2
  sbc rc6
  sta __st+2
  lda __sr+3
  sbc rc7
  sta __st+3
.endmacro

udivmod32:                      ; __sa / __sb -> __sa, __sa % __sb -> __sr (unsigned)
  lda #0
  sta __sr
  sta __sr+1
  sta __sr+2
  sta __sr+3
  ldx #32
@next:
  asl __sa
  rol __sa+1
  rol __sa+2
  rol __sa+3
  rol __sr
  rol __sr+1
  rol __sr+2
  rol __sr+3
  bcs @take                     ; the remainder overflowed 32 bits: it is above any divisor
  cmp_sub_32
  bcc @skip                     ; borrow: the remainder is below the divisor
@take:
  cmp_sub_32
  lda __st
  sta __sr
  lda __st+1
  sta __sr+1
  lda __st+2
  sta __sr+2
  lda __st+3
  sta __sr+3
  inc __sa                      ; the quotient bit (bit 0 of __sa is clear after the shift)
@skip:
  dex
  beq @done                     ; the loop is past a short branch's reach
  jmp @next
@done:
  rts

copy_sr_to_sa:
  lda __sr
  sta __sa
  lda __sr+1
  sta __sa+1
  lda __sr+2
  sta __sa+2
  lda __sr+3
  sta __sa+3
  rts

neg_sa:
  sec
  lda #0
  sbc __sa
  sta __sa
  lda #0
  sbc __sa+1
  sta __sa+1
  lda #0
  sbc __sa+2
  sta __sa+2
  lda #0
  sbc __sa+3
  sta __sa+3
  rts

neg_sb:
  sec
  lda #0
  sbc rc4
  sta rc4
  lda #0
  sbc rc5
  sta rc5
  lda #0
  sbc rc6
  sta rc6
  lda #0
  sbc rc7
  sta rc7
  rts

neg_sr:
  sec
  lda #0
  sbc __sr
  sta __sr
  lda #0
  sbc __sr+1
  sta __sr+1
  lda #0
  sbc __sr+2
  sta __sr+2
  lda #0
  sbc __sr+3
  sta __sr+3
  rts

divmod32s_signs:                ; the signs of the quotient (__sg bit 0) and the remainder (bit 1)
  lda __sg
  and #1
  beq @r
  jsr neg_sa
@r:
  lda __sg
  and #2
  beq @done
  jmp neg_sr
@done:
  rts

s32_udivsi3:                      ; __sa / __sb -> __sa
  jmp udivmod32

s32_umodsi3:                      ; __sa % __sb -> __sa
  jsr udivmod32
  jmp copy_sr_to_sa

; Signed: divide the magnitudes, then give the quotient the sign of a xor b and the remainder the
; sign of a (bit 0 of __sg is the quotient's sign, bit 1 the remainder's).
divmod32s:
  lda #0
  sta __sg
  lda __sa+3
  bpl @a
  lda #3
  sta __sg
  jsr neg_sa
@a:
  lda rc7
  bpl @b
  lda __sg
  eor #1
  sta __sg
  jsr neg_sb
@b:
  jmp udivmod32

s32_divsi3:                       ; __sa / __sb -> __sa (signed, truncated toward zero)
  jsr divmod32s
  lda __sg
  and #1
  beq @done
  jsr neg_sa
@done:
  rts

s32_modsi3:                       ; __sa % __sb -> __sa (signed, the sign of the dividend)
  jsr divmod32s
  jsr copy_sr_to_sa
  lda __sg
  and #2
  beq @done
  jsr neg_sa
@done:
  rts

s32_divmodsi4:                    ; __sa / __sb -> __sa, __sa % __sb -> __sr (signed, truncated toward zero)
  jsr divmod32s
  jmp divmod32s_signs

; ---- shifts of a long by a variable count, in place: A:X:rc2:rc3 shifted by rc4 ------------------------
; The low word walks in rc5:rc6 (rc4 is the count, which goes to X) and comes back in A:X. The speed
; variants (the compiler calls them at -O2) move whole bytes while the count is 8 or more, 38 clocks a
; byte step against 200 for eight bit steps, then the rest is shifted bit by bit. Same results as the bit
; loops for every count, 32 and more included.

.export __ashlsi3
__ashlsi3:                      ; A:X:rc2:rc3 << rc4 -> A:X:rc2:rc3
  sta rc5
  stx rc6
  ldx rc4
  beq @done
@next:
  asl rc5
  rol rc6
  rol rc2
  rol rc3
  dex
  bne @next
@done:
  lda rc5
  ldx rc6
  rts

.export __lshrsi3
__lshrsi3:                      ; A:X:rc2:rc3 >> rc4 (logical) -> A:X:rc2:rc3
  sta rc5
  stx rc6
  ldx rc4
  beq @done
@next:
  lsr rc3
  ror rc2
  ror rc6
  ror rc5
  dex
  bne @next
@done:
  lda rc5
  ldx rc6
  rts

.export __ashrsi3
__ashrsi3:                      ; A:X:rc2:rc3 >> rc4 (arithmetic) -> A:X:rc2:rc3
  sta rc5
  stx rc6
  ldx rc4
  beq @done
@next:
  lda rc3
  cmp #$80
  ror rc3
  ror rc2
  ror rc6
  ror rc5
  dex
  bne @next
@done:
  lda rc5
  ldx rc6
  rts

.export __ashlsi3_fast
__ashlsi3_fast:                 ; A:X:rc2:rc3 << rc4 -> A:X:rc2:rc3
  sta rc5
  stx rc6
  ldx rc4
  beq @done
@bytes:
  cpx #8
  bcc @bits
  lda rc2
  sta rc3
  lda rc6
  sta rc2
  lda rc5
  sta rc6
  lda #0
  sta rc5
  txa
  sec
  sbc #8
  tax
  bne @bytes
  beq @done
@bits:
  asl rc5
  rol rc6
  rol rc2
  rol rc3
  dex
  bne @bits
@done:
  lda rc5
  ldx rc6
  rts

.export __lshrsi3_fast
__lshrsi3_fast:                 ; A:X:rc2:rc3 >> rc4 (logical) -> A:X:rc2:rc3
  sta rc5
  stx rc6
  ldx rc4
  beq @done
@bytes:
  cpx #8
  bcc @bits
  lda rc6
  sta rc5
  lda rc2
  sta rc6
  lda rc3
  sta rc2
  lda #0
  sta rc3
  txa
  sec
  sbc #8
  tax
  bne @bytes
  beq @done
@bits:
  lsr rc3
  ror rc2
  ror rc6
  ror rc5
  dex
  bne @bits
@done:
  lda rc5
  ldx rc6
  rts

.export __ashrsi3_fast
__ashrsi3_fast:                 ; A:X:rc2:rc3 >> rc4 (arithmetic) -> A:X:rc2:rc3
  sta rc5
  stx rc6
  ldx rc4
  beq @done
@bytes:
  cpx #8
  bcc @bits
  lda rc6
  sta rc5
  lda rc2
  sta rc6
  lda rc3
  sta rc2
  cmp #$80                      ; the sign byte: carry is the sign
  lda #0
  sbc #0                        ; $FF for a positive value, 0 for a negative
  eor #$FF                      ; the fill: $FF for a negative value
  sta rc3
  txa
  sec
  sbc #8
  tax
  bne @bytes
  beq @done
@bits:
  lda rc3
  cmp #$80
  ror rc3
  ror rc2
  ror rc6
  ror rc5
  dex
  bne @bits
@done:
  lda rc5
  ldx rc6
  rts

; ---- memory fills and copies ----------------------------------------------------------------------
; memset(rc2:rc3 = dst, A = byte, rc4:rc5 = n), memcpy and memmove(rc2:rc3 = dst, rc4:rc5 = src, A:X = n):
; the pointers in cells (llvm-mos), n as a word; each returns dst in A:X. They walk whole pages with Y, so
; the low byte of dst stays put and only its high byte is saved (rc6, memcpy in rc7, memmove on the
; hardware stack).

.export memset
memset:
  ldx rc3
  stx rc6
  ldx rc5
  ldy #0
  cpx #0
  beq @tail
@page:
  sta (rc2),y
  iny
  bne @page
  inc rc3
  dex
  bne @page
@tail:
  ldx rc4
  beq @done
@byte:
  sta (rc2),y
  iny
  dex
  bne @byte
@done:
  ldx rc6
  lda rc2
  rts

.export memcpy
memcpy:
  sta rc6
  lda rc3
  sta rc7
  ldy #0
  cpx #0
  beq @tail
@page:
  lda (rc4),y
  sta (rc2),y
  iny
  bne @page
  inc rc5
  inc rc3
  dex
  bne @page
@tail:
  ldx rc6
  beq @done
@byte:
  lda (rc4),y
  sta (rc2),y
  iny
  dex
  bne @byte
@done:
  ldx rc7
  lda rc2
  rts

; dst below src copies upward; otherwise downward, from the last byte, which is safe whether or not
; the ranges overlap
.export memmove
memmove:
  sta rc6
  lda rc2
  cmp rc4
  lda rc3
  sbc rc5
  bcs @down
  lda rc6
  jmp memcpy
@down:
  lda rc3
  pha
  txa
  clc
  adc rc3
  sta rc3
  txa
  clc
  adc rc5
  sta rc5
  ldy rc6
  beq @pages
@tail:
  dey
  lda (rc4),y
  sta (rc2),y
  cpy #0
  bne @tail
@pages:
  cpx #0
  beq @done
@page:
  dec rc3
  dec rc5
@byte:
  dey
  lda (rc4),y
  sta (rc2),y
  cpy #0
  bne @byte
  dex
  bne @page
@done:
  pla
  tax
  lda rc2
  rts

; ---- speed variants of the 32-bit division (after the other routines: their addresses, and the
; page crossings the clock tests count, stay where they were) ----
udivmod32_fast_byte:                 ; __sa / __sb -> __sa, __sa % __sb -> __sr for a divisor of 1 to 255 (the caller
  lda #0                        ; knows it; a zero divisor gives the quotient all ones, the remainder unspecified)
  sta __sr+1                    ; the remainder stays in A, a byte at a time
  sta __sr+2
  sta __sr+3
  ldx #3
@byte:
  ldy #8
@bit:
  asl __sa,x
  rol a
  bcs @t                        ; the remainder overflowed 8 bits: it is above the divisor
  cmp rc4
  bcc @s
@t:
  sbc rc4                      ; C is set
  inc __sa,x                    ; the quotient bit (bit 0 is clear after the shift)
@s:
  dey
  bne @bit
  dex
  bpl @byte
  sta __sr
  rts

udivmod32_fast:                      ; the speed variant: __sa / __sb -> __sa, __sa % __sb -> __sr (unsigned)
  lda rc5                    ; a divisor below 256 takes the byte loop
  ora rc6
  ora rc7
  bne udivmod32_wide
  lda rc4
  beq udivmod32_wide            ; a zero divisor takes the wide path: its quotient is all ones
  jmp udivmod32_fast_byte

udivmod32_wide:
  lda rc4                      ; a zero divisor: all-ones quotient, the dividend as remainder
  ora rc5
  ora rc6
  ora rc7
  bne @nz
  ldx #3
@zero:
  lda __sa,x
  sta __sr,x
  lda #$FF
  sta __sa,x
  dex
  bpl @zero
  rts
@nz:
  lda __sa                      ; the remainder starts as the dividend
  sta __sr
  lda __sa+1
  sta __sr+1
  lda __sa+2
  sta __sr+2
  lda __sa+3
  sta __sr+3
  ldx #3                        ; a divisor above the dividend: quotient 0
@cmpd:
  lda __sa,x
  cmp rc4,x; [rc4..rc7]
  bne @cmpe
  dex
  bpl @cmpd
  inx                           ; equal: C is set
@cmpe:
  bcs @big
  lda #0
  sta __sa
  sta __sa+1
  sta __sa+2
  sta __sa+3
  rts
; The pin's compiler-rt `__udivXi3` (int_div_impl.inc) runs for the quotient's bit length. The
; divisor is shifted left while it stays at or below half the dividend (X counts the shifts), then
; each trip subtracts it from the remainder when it fits, shifts that answer into the quotient
; and halves the divisor: X + 1 trips, one for each bit of the quotient.
@big:
  lda __sa+3                    ; half the dividend, in __st
  lsr a
  sta __st+3
  lda __sa+2
  ror a
  sta __st+2
  lda __sa+1
  ror a
  sta __st+1
  lda __sa
  ror a
  sta __st
  ldx #0
@abyte:                         ; a whole byte at once while the divisor shifted by 8 stays at or below half
  lda rc7
  bne @align
  lda rc6
  cmp __st+3
  bne @cb
  lda rc5
  cmp __st+2
  bne @cb
  lda rc4
  cmp __st+1
  beq @bshift                   ; equal above, and the divisor's low byte is 0 after the shift
@cb:
  bcs @align
@bshift:
  lda rc6
  sta rc7
  lda rc5
  sta rc6
  lda rc4
  sta rc5
  lda #0
  sta rc4
  txa
  clc
  adc #8
  tax
  bne @abyte
@align:
  lda rc7                    ; the divisor above half the dividend: aligned
  cmp __st+3
  bne @cmph
  lda rc6
  cmp __st+2
  bne @cmph
  lda rc5
  cmp __st+1
  bne @cmph
  lda rc4
  cmp __st
  beq @shift
@cmph:
  bcs @aligned
@shift:
  asl rc4
  rol rc5
  rol rc6
  rol rc7
  inx
  bne @align                    ; X is never 0 here: the loop is past a short branch's reach otherwise
@aligned:
  lda #0
  sta __sa
  sta __sa+1
  sta __sa+2
  sta __sa+3
  inx
@trip:
  lda __sr+3                    ; compare from the high byte: most trips end at the first
  cmp rc7
  bne @c2
  lda __sr+2
  cmp rc6
  bne @c2
  lda __sr+1
  cmp rc5
  bne @c2
  lda __sr
  cmp rc4
@c2:
  bcc @no                       ; the remainder is below the divisor: the quotient bit is 0 (C clear)
  lda __sr                      ; C is set: no borrow, and the quotient bit is 1
  sbc rc4
  sta __sr
  lda __sr+1
  sbc rc5
  sta __sr+1
  lda __sr+2
  sbc rc6
  sta __sr+2
  lda __sr+3
  sbc rc7
  sta __sr+3
@no:
  rol __sa                      ; the quotient bit comes in at the bottom
  rol __sa+1
  rol __sa+2
  rol __sa+3
  lsr rc7
  ror rc6
  ror rc5
  ror rc4
  dex
  bne @trip
  rts

s32_udivsi3_fast:                 ; __sa / __sb -> __sa
  jmp udivmod32_fast

s32_udivsi3_fast_byte:            ; __sa / __sb -> __sa, a divisor of 1 to 255
  jmp udivmod32_fast_byte

s32_umodsi3_fast:                 ; __sa % __sb -> __sa
  jmp umod32_fast

s32_umodsi3_fast_byte:            ; __sa % __sb -> __sa, a divisor of 1 to 255
  jsr udivmod32_fast_byte
  jmp copy_sr_to_sa

divmod32s_fast:                 ; the signs split off, then the speed division
  lda #0
  sta __sg
  lda __sa+3
  bpl @a
  lda #3
  sta __sg
  jsr neg_sa
@a:
  lda rc7
  bpl @b
  lda __sg
  eor #1
  sta __sg
  jsr neg_sb
@b:
  jmp udivmod32_fast

s32_divsi3_fast:                  ; __sa / __sb -> __sa (signed, truncated toward zero)
  jsr divmod32s_fast
  lda __sg
  and #1
  beq @done
  jsr neg_sa
@done:
  rts

s32_modsi3_fast:                  ; __sa % __sb -> __sa (signed, the sign of the dividend)
  jmp umod32s_fast

divmod32s_fast_byte:            ; the dividend's sign split off (the divisor is 1 to 255), then the byte loop
  lda #0
  sta __sg
  lda __sa+3
  bpl @a
  lda #3
  sta __sg
  jsr neg_sa
@a:
  jmp udivmod32_fast_byte

s32_divsi3_fast_byte:             ; __sa / __sb -> __sa (signed), a divisor of 1 to 255
  jsr divmod32s_fast_byte
  lda __sg
  and #1
  beq @done
  jsr neg_sa
@done:
  rts

s32_modsi3_fast_byte:             ; __sa % __sb -> __sa (signed), a divisor of 1 to 255
  jsr divmod32s_fast_byte
  jsr copy_sr_to_sa
  lda __sg
  and #2
  beq @done
  jsr neg_sa
@done:
  rts

s32_divmodsi4_fast:               ; __sa / __sb -> __sa, __sa % __sb -> __sr (signed)
  jsr divmod32s_fast
  jmp divmod32s_signs

s32_divmodsi4_fast_byte:          ; __sa / __sb -> __sa, __sa % __sb -> __sr (signed), a divisor of 1 to 255
  jsr divmod32s_fast_byte
  jmp divmod32s_signs

; ---- the speed division with the small wide path: the routines of the module that divides cold
; (sites that run fewer than 3 times together, `hotdivision`) are the ones of the first speed variant,
; byte for byte: the pin-structure wide path above is 141 bytes more and pays only where it runs ----
udivmod32_fast_small:                      ; the speed variant: __sa / __sb -> __sa, __sa % __sb -> __sr (unsigned)
  lda #0
  sta __sr
  sta __sr+1
  sta __sr+2
  sta __sr+3
  lda rc5                    ; a divisor below 256: the remainder stays in A, a byte at a time
  ora rc6
  ora rc7
  bne @wide
  lda rc4
  beq @wide                     ; a zero divisor takes the general path: its quotient is all ones
  lda #0
  ldx #3
@byte:
  ldy #8
@bit:
  asl __sa,x
  rol a
  bcs @t                        ; the remainder overflowed 8 bits: it is above the divisor
  cmp rc4
  bcc @s
@t:
  sbc rc4                      ; C is set
  inc __sa,x                    ; the quotient bit (bit 0 is clear after the shift)
@s:
  dey
  bne @bit
  dex
  bpl @byte
  sta __sr
  rts
@wide:
  ldx #32
; While the remainder with the dividend's next byte appended is still below the divisor, the
; eight trips for that byte only shift it in: take the byte at once. A divisor of m bytes lets
; m-1 bytes go this way, a dividend of fewer bytes than that is all remainder, and a zero divisor
; never passes the test, so its quotient is all ones.
@skip:
  lda __sr+3
  bne @next
  lda __sr+2
  cmp rc7
  bne @below
  lda __sr+1
  cmp rc6
  bne @below
  lda __sr
  cmp rc5
  bne @below
  lda __sa+3
  cmp rc4
@below:
  bcs @next                     ; the remainder reaches the divisor: the trips start here
  lda __sr+2
  sta __sr+3
  lda __sr+1
  sta __sr+2
  lda __sr
  sta __sr+1
  lda __sa+3
  sta __sr
  lda __sa+2
  sta __sa+3
  lda __sa+1
  sta __sa+2
  lda __sa
  sta __sa+1
  lda #0
  sta __sa
  txa
  sec
  sbc #8
  tax
  bne @skip
  rts                           ; the whole dividend is remainder
@next:
  asl __sa
  rol __sa+1
  rol __sa+2
  rol __sa+3
  rol __sr
  rol __sr+1
  rol __sr+2
  rol __sr+3
  bcs @take                     ; the remainder overflowed 32 bits: it is above any divisor
  lda __sr+3                    ; compare from the high byte: most trips end at the first
  cmp rc7
  bne @cmp
  lda __sr+2
  cmp rc6
  bne @cmp
  lda __sr+1
  cmp rc5
  bne @cmp
  lda __sr
  cmp rc4
@cmp:
  bcc @skipbit                  ; the remainder is below the divisor
@take:
  sec
  lda __sr
  sbc rc4
  sta __sr
  lda __sr+1
  sbc rc5
  sta __sr+1
  lda __sr+2
  sbc rc6
  sta __sr+2
  lda __sr+3
  sbc rc7
  sta __sr+3
  inc __sa                      ; the quotient bit (bit 0 of __sa is clear after the shift)
@skipbit:
  dex
  bne @next
  rts

s32_udivsi3_fast_small:                 ; __sa / __sb -> __sa
  jmp udivmod32_fast_small

s32_umodsi3_fast_small:                 ; __sa % __sb -> __sa
  jsr udivmod32_fast_small
  jmp copy_sr_to_sa

divmod32s_fast_small:                 ; the signs split off, then the speed division
  lda #0
  sta __sg
  lda __sa+3
  bpl @a
  lda #3
  sta __sg
  jsr neg_sa
@a:
  lda rc7
  bpl @b
  lda __sg
  eor #1
  sta __sg
  jsr neg_sb
@b:
  jmp udivmod32_fast_small

s32_divsi3_fast_small:                  ; __sa / __sb -> __sa (signed, truncated toward zero)
  jsr divmod32s_fast_small
  lda __sg
  and #1
  beq @done
  jsr neg_sa
@done:
  rts

s32_modsi3_fast_small:                  ; __sa % __sb -> __sa (signed, the sign of the dividend)
  jsr divmod32s_fast_small
  jsr copy_sr_to_sa
  lda __sg
  and #2
  beq @done
  jsr neg_sa
@done:
  rts

s32_divmodsi4_fast_small:         ; __sa / __sb -> __sa, __sa % __sb -> __sr (signed)
  jsr divmod32s_fast_small
  jmp divmod32s_signs

; ---- the speed remainder: the quotient is never built, as in llvm-mos's `__umodsi3`. The dividend is
; the remainder, in place in __sa; the divisor is shifted up as far as the quotient's first bit, and each
; trip subtracts it when it fits and halves it ----
umod32_fast:                         ; __sa % __sb -> __sa (unsigned)
  lda rc4
  ora rc5
  ora rc6
  ora rc7
  beq @zero                     ; a zero divisor: the dividend is the remainder
  lda rc5                    ; a divisor below 256 takes the byte loop
  ora rc6
  ora rc7
  bne umod32_wide
  jsr udivmod32_fast_byte
  jmp copy_sr_to_sa
@zero:
  rts

umod32_wide:
  ldx #3                        ; a divisor above the dividend: the dividend is the remainder
@cmpd:
  lda __sa,x
  cmp rc4,x; [rc4..rc7]
  bne @cmpe
  dex
  bpl @cmpd
  inx                           ; equal: C is set
@cmpe:
  bcs @big
  rts
@big:
  lda __sa+3                    ; half the dividend, in __st
  lsr a
  sta __st+3
  lda __sa+2
  ror a
  sta __st+2
  lda __sa+1
  ror a
  sta __st+1
  lda __sa
  ror a
  sta __st
  ldx #0
@abyte:                         ; a whole byte at once while the divisor shifted by 8 stays at or below half
  lda rc7
  bne @align
  lda rc6
  cmp __st+3
  bne @cb
  lda rc5
  cmp __st+2
  bne @cb
  lda rc4
  cmp __st+1
  beq @bshift                   ; equal above, and the divisor's low byte is 0 after the shift
@cb:
  bcs @align
@bshift:
  lda rc6
  sta rc7
  lda rc5
  sta rc6
  lda rc4
  sta rc5
  lda #0
  sta rc4
  txa
  clc
  adc #8
  tax
  bne @abyte
@align:
  lda rc7                    ; the divisor above half the dividend: aligned
  cmp __st+3
  bne @cmph
  lda rc6
  cmp __st+2
  bne @cmph
  lda rc5
  cmp __st+1
  bne @cmph
  lda rc4
  cmp __st
  beq @shift
@cmph:
  bcs @aligned
@shift:
  asl rc4
  rol rc5
  rol rc6
  rol rc7
  inx
  bne @align                    ; X is never 0 here: the loop is past a short branch's reach otherwise
@aligned:
  inx
@trip:
  lda __sa+3                    ; compare from the high byte: most trips end at the first
  cmp rc7
  bne @c2
  lda __sa+2
  cmp rc6
  bne @c2
  lda __sa+1
  cmp rc5
  bne @c2
  lda __sa
  cmp rc4
@c2:
  bcc @no                       ; the remainder is below the divisor: the quotient bit is 0 (C clear)
  lda __sa                      ; C is set: no borrow, and the quotient bit is 1
  sbc rc4
  sta __sa
  lda __sa+1
  sbc rc5
  sta __sa+1
  lda __sa+2
  sbc rc6
  sta __sa+2
  lda __sa+3
  sbc rc7
  sta __sa+3
@no:
  lsr rc7
  ror rc6
  ror rc5
  ror rc4
  dex
  bne @trip
  rts


umod32s_fast:                        ; __sa % __sb -> __sa (signed, the sign of the dividend)
  lda #0
  sta __sg
  lda __sa+3
  bpl @a
  lda #2
  sta __sg
  jsr neg_sa
@a:
  lda rc7
  bpl @b
  jsr neg_sb
@b:
  jsr umod32_fast
  lda __sg
  beq @done
  jmp neg_sa
@done:
  rts

; ---- save helpers that keep A and X (last in the file: no routine above moves) ----------------------

; __csave_ax and __crest_ax: the same for a function that has a value in A and X (a word argument at
; entry, a word result at return), the way RISC-V's save-restore libcalls leave the argument registers
; alone. Y = n and is changed; A and X come back as they went in.
.export __csave_ax
__csave_ax:
  sta __sa
  stx __sa+1
  pla
  sta __ra
  pla
  sta __ra+1
  ldx #1
@next:
  lda rc7,x                     ; [rc8..rc15]
  pha
  inx
  dey
  bne @next
  lda __ra+1
  pha
  lda __ra
  pha
  lda __sa
  ldx __sa+1
  rts

.export __crest_ax
__crest_ax:
  sta __sa
  stx __sa+1
  tya
  tax
@next:
  pla
  sta rc7,x                     ; [rc8..rc15]
  dex
  bne @next
  lda __sa
  ldx __sa+1
  rts

; ---- the lean speed division (last in the file: no routine above moves): the small speed division with the
; dividend's byte skip a loop over {__sa, __sr}, 31 bytes fewer for 43 clocks more a skipped byte. The calls
; of modules that divide cold at the speed levels (`hotdivision`) are these; `_fast_small` stays the unrolled
; one that the size levels choose where the division is hot ----

.export __udivsi3_fast_lean
__udivsi3_fast_lean:
  jsr in32
  jsr s32_udivsi3_fast_lean
  jmp out32

.export __umodsi3_fast_lean
__umodsi3_fast_lean:
  jsr in32
  jsr s32_umodsi3_fast_lean
  jmp out32

.export __divsi3_fast_lean
__divsi3_fast_lean:
  jsr in32
  jsr s32_divsi3_fast_lean
  jmp out32

.export __modsi3_fast_lean
__modsi3_fast_lean:
  jsr in32
  jsr s32_modsi3_fast_lean
  jmp out32

.export __divmodsi4_fast_lean
__divmodsi4_fast_lean:
  jsr in32
  jsr s32_divmodsi4_fast_lean
  jmp out32r

.export __udivmodsi4_fast_lean
__udivmodsi4_fast_lean:
  jsr in32
  jsr s32_udivsi3_fast_lean
  jmp out32r

udivmod32_fast_lean:                      ; the speed variant: __sa / __sb -> __sa, __sa % __sb -> __sr (unsigned)
  lda #0
  sta __sr
  sta __sr+1
  sta __sr+2
  sta __sr+3
  lda rc5                    ; a divisor below 256: the remainder stays in A, a byte at a time
  ora rc6
  ora rc7
  bne @wide
  lda rc4
  beq @wide                     ; a zero divisor takes the general path: its quotient is all ones
  lda #0
  ldx #3
@byte:
  ldy #8
@bit:
  asl __sa,x
  rol a
  bcs @t                        ; the remainder overflowed 8 bits: it is above the divisor
  cmp rc4
  bcc @s
@t:
  sbc rc4                      ; C is set
  inc __sa,x                    ; the quotient bit (bit 0 is clear after the shift)
@s:
  dey
  bne @bit
  dex
  bpl @byte
  sta __sr
  rts
@wide:
  ldx #32
; While the remainder with the dividend's next byte appended is still below the divisor, the
; eight trips for that byte only shift it in: take the byte at once. A divisor of m bytes lets
; m-1 bytes go this way, a dividend of fewer bytes than that is all remainder, and a zero divisor
; never passes the test, so its quotient is all ones.
@skip:
  lda __sr+3
  bne @next
  lda __sr+2
  cmp rc7
  bne @below
  lda __sr+1
  cmp rc6
  bne @below
  lda __sr
  cmp rc5
  bne @below
  lda __sa+3
  cmp rc4
@below:
  bcs @next                     ; the remainder reaches the divisor: the trips start here
  ldy #6                        ; {__sr, __sa} one byte up, the high byte of __sr out
@up:
  lda __sa,y
  sta __sa+1,y
  dey
  bpl @up
  lda #0
  sta __sa
  txa
  sec
  sbc #8
  tax
  bne @skip
  rts                           ; the whole dividend is remainder
@next:
  asl __sa
  rol __sa+1
  rol __sa+2
  rol __sa+3
  rol __sr
  rol __sr+1
  rol __sr+2
  rol __sr+3
  bcs @take                     ; the remainder overflowed 32 bits: it is above any divisor
  lda __sr+3                    ; compare from the high byte: most trips end at the first
  cmp rc7
  bne @cmp
  lda __sr+2
  cmp rc6
  bne @cmp
  lda __sr+1
  cmp rc5
  bne @cmp
  lda __sr
  cmp rc4
@cmp:
  bcc @skipbit                  ; the remainder is below the divisor
@take:
  sec
  lda __sr
  sbc rc4
  sta __sr
  lda __sr+1
  sbc rc5
  sta __sr+1
  lda __sr+2
  sbc rc6
  sta __sr+2
  lda __sr+3
  sbc rc7
  sta __sr+3
  inc __sa                      ; the quotient bit (bit 0 of __sa is clear after the shift)
@skipbit:
  dex
  bne @next
  rts

s32_udivsi3_fast_lean:                 ; __sa / __sb -> __sa
  jmp udivmod32_fast_lean

s32_umodsi3_fast_lean:                 ; __sa % __sb -> __sa
  jsr udivmod32_fast_lean
  jmp copy_sr_to_sa

divmod32s_fast_lean:                 ; the signs split off, then the speed division
  lda #0
  sta __sg
  lda __sa+3
  bpl @a
  lda #3
  sta __sg
  jsr neg_sa
@a:
  lda rc7
  bpl @b
  lda __sg
  eor #1
  sta __sg
  jsr neg_sb
@b:
  jmp udivmod32_fast_lean

s32_divsi3_fast_lean:                  ; __sa / __sb -> __sa (signed, truncated toward zero)
  jsr divmod32s_fast_lean
  lda __sg
  and #1
  beq @done
  jsr neg_sa
@done:
  rts

s32_modsi3_fast_lean:                  ; __sa % __sb -> __sa (signed, the sign of the dividend)
  jsr divmod32s_fast_lean
  jsr copy_sr_to_sa
  lda __sg
  and #2
  beq @done
  jsr neg_sa
@done:
  rts

s32_divmodsi4_fast_lean:         ; __sa / __sb -> __sa, __sa % __sb -> __sr (signed)
  jsr divmod32s_fast_lean
  jmp divmod32s_signs

; ---- the multiply by a byte (last in the file: no routine above moves): A:X:rc2:rc3 * rc4, rc5..rc7 not read
; (the caller does not set them). The compact walk is 8 trips; the speed walk stops at the multiplier's last set bit ----
.export __mulsi3_byte
__mulsi3_byte:
  sta __sa
  stx __sa+1
  lda #0
  sta __st
  sta __st+1
  sta __st+2
  sta __st+3
  ldx #8
@next:
  lsr rc4
  bcc @skip
  clc
  lda __st
  adc __sa
  sta __st
  lda __st+1
  adc __sa+1
  sta __st+1
  lda __st+2
  adc rc2
  sta __st+2
  lda __st+3
  adc rc3
  sta __st+3
@skip:
  asl __sa
  rol __sa+1
  rol rc2
  rol rc3
  dex
  bne @next
  jmp out32_st

.export __mulsi3_fast_byte
__mulsi3_fast_byte:
  sta __sa
  stx __sa+1
  lda #0
  sta __st
  sta __st+1
  sta __st+2
  sta __st+3
@next:
  lsr rc4
  bcc @skip
  clc
  lda __st
  adc __sa
  sta __st
  lda __st+1
  adc __sa+1
  sta __st+1
  lda __st+2
  adc rc2
  sta __st+2
  lda __st+3
  adc rc3
  sta __st+3
@skip:
  asl __sa
  rol __sa+1
  rol rc2
  rol rc3
  lda rc4
  bne @next
  jmp out32_st
