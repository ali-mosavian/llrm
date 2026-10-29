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
  lda rc7,x
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
  sta rc7,x
  dex
  bne @next
  tya
  rts

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

; rs1 / rs2: the quotient in rc2:rc3, the remainder in rc6:rc7. Uses A and X only.
udivmod16:
  lda rc5
  bne @wide
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
@wide:
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

; ---- 32-bit operations -------------------------------------------------------------------------
; The compiler copies the operands to __sa (first) and __sb (second, or a shift count in its first
; byte) and reads the result from __sa. The routines change A, X, Y, __sa, __sb, __st, __sr and
; __sg and nothing else. Division by zero gives the all-ones quotient and the dividend.

.segment "BSS"
__sa: .res 4
__ra: .res 2
__sb: .res 4
__st: .res 4
__sr: .res 4
__sg: .res 1
.segment "CODE"

.export __mulsi3
__mulsi3:                       ; __sa * __sb -> __sa (the low 32 bits)
  lda #0
  sta __st
  sta __st+1
  sta __st+2
  sta __st+3
  ldx #32
@next:
  lsr __sb+3
  ror __sb+2
  ror __sb+1
  ror __sb
  bcc @skip
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
  lda __st+3
  adc __sa+3
  sta __st+3
@skip:
  asl __sa
  rol __sa+1
  rol __sa+2
  rol __sa+3
  dex
  bne @next
  jmp copy_st_to_sa

.macro cmp_sub_32
  sec
  lda __sr
  sbc __sb
  sta __st
  lda __sr+1
  sbc __sb+1
  sta __st+1
  lda __sr+2
  sbc __sb+2
  sta __st+2
  lda __sr+3
  sbc __sb+3
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

copy_st_to_sa:
  lda __st
  sta __sa
  lda __st+1
  sta __sa+1
  lda __st+2
  sta __sa+2
  lda __st+3
  sta __sa+3
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
  sbc __sb
  sta __sb
  lda #0
  sbc __sb+1
  sta __sb+1
  lda #0
  sbc __sb+2
  sta __sb+2
  lda #0
  sbc __sb+3
  sta __sb+3
  rts

.export __udivsi3
__udivsi3:                      ; __sa / __sb -> __sa
  jmp udivmod32

.export __umodsi3
__umodsi3:                      ; __sa % __sb -> __sa
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
  lda __sb+3
  bpl @b
  lda __sg
  eor #1
  sta __sg
  jsr neg_sb
@b:
  jmp udivmod32

.export __divsi3
__divsi3:                       ; __sa / __sb -> __sa (signed, truncated toward zero)
  jsr divmod32s
  lda __sg
  and #1
  beq @done
  jsr neg_sa
@done:
  rts

.export __modsi3
__modsi3:                       ; __sa % __sb -> __sa (signed, the sign of the dividend)
  jsr divmod32s
  jsr copy_sr_to_sa
  lda __sg
  and #2
  beq @done
  jsr neg_sa
@done:
  rts

.export __ashlsi3
__ashlsi3:                      ; __sa << __sb (low byte) -> __sa
  ldx __sb
  beq @done
@next:
  asl __sa
  rol __sa+1
  rol __sa+2
  rol __sa+3
  dex
  bne @next
@done:
  rts

.export __lshrsi3
__lshrsi3:                      ; __sa >> __sb (logical) -> __sa
  ldx __sb
  beq @done
@next:
  lsr __sa+3
  ror __sa+2
  ror __sa+1
  ror __sa
  dex
  bne @next
@done:
  rts

.export __ashrsi3
__ashrsi3:                      ; __sa >> __sb (arithmetic) -> __sa
  ldx __sb
  beq @done
@next:
  lda __sa+3
  cmp #$80
  ror __sa+3
  ror __sa+2
  ror __sa+1
  ror __sa
  dex
  bne @next
@done:
  rts
