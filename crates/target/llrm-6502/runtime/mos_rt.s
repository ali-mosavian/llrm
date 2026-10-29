; Spike runtime for the 6502 target: what the compiler would call for multiply, divide, remainder and
; shifts by a variable. The `mos` convention: a byte argument in the first free of rc2.., a word in
; rs1 (rc2:rc3) then rs2 (rc4:rc5), a byte result in A, a word in A (low):X (high). The routines may
; change A, X, Y and rc2..rc7 and keep rc0, rc1 and rc8..rc15. The caller defines rc0..rc15 (the
; platform's zero-page base).
;
; Division by zero gives the all-ones quotient and the dividend as remainder (the restoring loop
; below, unchanged). Signed division truncates toward zero and the remainder has the dividend's sign.

; ---- multiply: the low half of the product -------------------------------------------------------

.export __mulqi3
__mulqi3:                       ; rc2 * rc3 -> A
  lda #0
  ldx #8
@next:
  lsr rc3
  bcc @skip
  clc
  adc rc2
@skip:
  asl rc2
  dex
  bne @next
  rts

.export __mulhi3
__mulhi3:                       ; rs1 * rs2 -> A:X
  lda #0
  sta rc6
  sta rc7
  ldx #16
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
  dex
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

; rs1 / rs2: the quotient in rc2:rc3, the remainder in rc6:rc7. Uses A and X only.
udivmod16:
  lda #0
  sta rc6
  sta rc7
  ldx #16
@next:
  asl rc2
  rol rc3
  rol rc6
  rol rc7
  bcs @sub
  lda rc6
  cmp rc4
  lda rc7
  sbc rc5
  bcc @keep
@sub:
  sec
  lda rc6
  sbc rc4
  sta rc6
  lda rc7
  sbc rc5
  sta rc7
  inc rc2
@keep:
  dex
  bne @next
  rts

.export __udivqi3
__udivqi3:                      ; rc2 / rc3 -> A
  jsr udivmod8
  lda rc2
  rts

.export __umodqi3
__umodqi3:                      ; rc2 % rc3 -> A
  jmp udivmod8

.export __udivhi3
__udivhi3:                      ; rs1 / rs2 -> A:X
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

; rs1 := |rs1|, rs2 := |rs2|
abs16_pair:
  lda rc3
  bpl @b
  neg16 rc2, rc3
@b:
  lda rc5
  bpl @done
  neg16 rc4, rc5
@done:
  rts

.export __divhi3
__divhi3:                       ; rs1 / rs2 -> A:X
  lda rc3
  eor rc5
  tay
  jsr abs16_pair
  jsr udivmod16
  cpy #0
  bpl @done
  neg16 rc2, rc3
@done:
  lda rc2
  ldx rc3
  rts

.export __modhi3
__modhi3:                       ; rs1 % rs2 -> A:X
  ldy rc3
  jsr abs16_pair
  jsr udivmod16
  cpy #0
  bpl @done
  neg16 rc6, rc7
@done:
  lda rc6
  ldx rc7
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
