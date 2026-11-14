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

; rs1 / rs2: the quotient in rc2:rc3, the remainder in rc6:rc7. Uses A and X only.
udivmod16:
  lda rc5
  bne udivmod16c
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
; the bit loop alone: what the compact entry points run
udivmod16c:
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

; the compact variants, called where a byte is worth more than the clocks: no short-divisor path
.export __udivhi3c
__udivhi3c:                     ; rs1 / rs2 -> A:X, the remainder in rc6:rc7
  jsr udivmod16c
  lda rc2
  ldx rc3
  rts

.export __umodhi3c
__umodhi3c:                     ; rs1 % rs2 -> A:X
  jsr udivmod16c
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

; the compact signed variants, called at size: the same sign handling over the bit loop alone (no short-divisor path)
.export __divhi3c
__divhi3c:                       ; rs1 / rs2 -> A:X, compact
  lda rc3
  eor rc5
  tay
  jsr abs16_pair
  jsr udivmod16c
  cpy #0
  bpl @done
  neg16 rc2, rc3
@done:
  lda rc2
  ldx rc3
  rts

.export __modhi3c
__modhi3c:                       ; rs1 % rs2 -> A:X, compact
  ldy rc3
  jsr abs16_pair
  jsr udivmod16c
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

; __mulsi3 for speed: the multiplier is walked a byte at a time and the walk
; stops at its last set byte, 19 bytes more, a third of the clocks for a byte.
.export __mulsi3_fast
__mulsi3_fast:                  ; __sa * __sb -> __sa (the low 32 bits)
  lda #0
  sta __st
  sta __st+1
  sta __st+2
  sta __st+3
  ldy #4
@byte:
  ldx #8
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
  lda __sb
  ora __sb+1
  ora __sb+2
  ora __sb+3
  beq @done
  dey
  bne @byte
@done:
  jmp copy_st_to_sa

; 16 x 16 -> 32: __sa and __sb low words in, the product in __sa. The product is built
; in __st:__sb, the multiplier shifting out of __sb as the low half shifts in. A multiplier
; whose high byte is zero is spent after 8 bits: the product is then the 32 bits shifted
; down a byte.
.export __umulhisi3
__umulhisi3:
  lda #0
  sta __st
  sta __st+1
  ldy #2
@pass:
  ldx #8
@next:
  lda __sb
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
  ror __sb+1
  ror __sb
  dex
  bne @next
  dey
  beq @long
  lda __sb
  bne @pass
  lda __sb+1
  sta __sa
  lda __st
  sta __sa+1
  lda __st+1
  sta __sa+2
  lda #0
  sta __sa+3
  rts
@long:
  lda __sb
  sta __sa
  lda __sb+1
  sta __sa+1
  lda __st
  sta __sa+2
  lda __st+1
  sta __sa+3
  rts

; the signed product: the unsigned one less the other operand from the high word for each
; negative operand
.export __mulhisi3
__mulhisi3:
  lda __sa
  sta __sr
  lda __sa+1
  sta __sr+1
  lda __sb
  sta __sr+2
  lda __sb+1
  sta __sr+3
  jsr __umulhisi3
  lda __sr+1
  bpl @a
  sec
  lda __sa+2
  sbc __sr+2
  sta __sa+2
  lda __sa+3
  sbc __sr+3
  sta __sa+3
@a:
  lda __sr+3
  bpl @done
  sec
  lda __sa+2
  sbc __sr
  sta __sa+2
  lda __sa+3
  sbc __sr+1
  sta __sa+3
@done:
  rts

; the same for a multiplier under 256 (its high byte known zero at the call): the multiplicand shifts
; left into 24 bits and is added for each set bit, and the walk ends with the multiplier's last set bit
.export __umulhisi3_byte
__umulhisi3_byte:
  lda #0
  sta __st
  sta __st+1
  sta __st+2
  sta __sa+2
@bit:
  lsr __sb
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
  lda __sb
  beq @out
  asl __sa
  rol __sa+1
  rol __sa+2
  jmp @bit
@out:
  lda __st
  sta __sa
  lda __st+1
  sta __sa+1
  lda __st+2
  sta __sa+2
  lda #0
  sta __sa+3
  rts

; the signed multiplicand by a byte: the unsigned product less the byte from the high word if the
; multiplicand is negative
.export __mulhisi3_byte
__mulhisi3_byte:
  lda __sb
  sta __sr
  lda __sa+1
  sta __sr+1
  jsr __umulhisi3_byte
  lda __sr+1
  bpl @done
  sec
  lda __sa+2
  sbc __sr
  sta __sa+2
  lda __sa+3
  sbc #0
  sta __sa+3
@done:
  rts

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

; The speed variants (the compiler calls them at -O2): whole bytes move while
; the count is 8 or more, 38 clocks a byte step against 200 for eight bit steps,
; then the rest is shifted bit by bit. Same results as the bit loops for every
; count, 32 and more included.
.export __ashlsi3_fast
__ashlsi3_fast:                 ; __sa << __sb (low byte) -> __sa
  ldx __sb
  beq @done
@bytes:
  cpx #8
  bcc @bits
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
  bne @bytes
  rts
@bits:
  asl __sa
  rol __sa+1
  rol __sa+2
  rol __sa+3
  dex
  bne @bits
@done:
  rts

.export __lshrsi3_fast
__lshrsi3_fast:                 ; __sa >> __sb (logical) -> __sa
  ldx __sb
  beq @done
@bytes:
  cpx #8
  bcc @bits
  lda __sa+1
  sta __sa
  lda __sa+2
  sta __sa+1
  lda __sa+3
  sta __sa+2
  lda #0
  sta __sa+3
  txa
  sec
  sbc #8
  tax
  bne @bytes
  rts
@bits:
  lsr __sa+3
  ror __sa+2
  ror __sa+1
  ror __sa
  dex
  bne @bits
@done:
  rts

.export __ashrsi3_fast
__ashrsi3_fast:                 ; __sa >> __sb (arithmetic) -> __sa
  ldx __sb
  beq @done
@bytes:
  cpx #8
  bcc @bits
  lda __sa+1
  sta __sa
  lda __sa+2
  sta __sa+1
  lda __sa+3
  sta __sa+2
  cmp #$80                      ; the sign byte: carry is the sign
  lda #0
  sbc #0                        ; $FF for a positive value, 0 for a negative
  eor #$FF                      ; the fill: $FF for a negative value
  sta __sa+3
  txa
  sec
  sbc #8
  tax
  bne @bytes
  rts
@bits:
  lda __sa+3
  cmp #$80
  ror __sa+3
  ror __sa+2
  ror __sa+1
  ror __sa
  dex
  bne @bits
@done:
  rts

; ---- memory fills and copies ----------------------------------------------------------------------
; memset(A:X = dst, rc2 = byte, rc4:rc5 = n), memcpy and memmove(A:X = dst, rc2:rc3 = src, rc4:rc5 = n);
; each returns dst in A:X. They walk whole pages with Y, so the low byte of dst stays put and only its
; high byte is saved (memset in rc3, the others on the hardware stack). The destination pointer is rc6:rc7.

.export memset
memset:
  sta rc6
  stx rc7
  stx rc3
  lda rc2
  ldy #0
  ldx rc5
  beq @tail
@page:
  sta (rc6),y
  iny
  bne @page
  inc rc7
  dex
  bne @page
@tail:
  ldx rc4
  beq @done
@byte:
  sta (rc6),y
  iny
  dex
  bne @byte
@done:
  ldx rc3
  lda rc6
  rts

.export memcpy
memcpy:
  sta rc6
  stx rc7
  txa
  pha
  ldy #0
  ldx rc5
  beq @tail
@page:
  lda (rc2),y
  sta (rc6),y
  iny
  bne @page
  inc rc3
  inc rc7
  dex
  bne @page
@tail:
  ldx rc4
  beq @done
@byte:
  lda (rc2),y
  sta (rc6),y
  iny
  dex
  bne @byte
@done:
  pla
  tax
  lda rc6
  rts

; dst below src copies upward; otherwise downward, from the last byte, which is safe whether or not
; the ranges overlap
.export memmove
memmove:
  sta rc6
  stx rc7
  cmp rc2
  txa
  sbc rc3
  bcs @down
  lda rc6
  jmp memcpy
@down:
  lda rc7
  pha
  clc
  adc rc5
  sta rc7
  lda rc3
  clc
  adc rc5
  sta rc3
  ldy rc4
  beq @pages
@tail:
  dey
  lda (rc2),y
  sta (rc6),y
  cpy #0
  bne @tail
@pages:
  ldx rc5
  beq @done
@page:
  dec rc7
  dec rc3
@byte:
  dey
  lda (rc2),y
  sta (rc6),y
  cpy #0
  bne @byte
  dex
  bne @page
@done:
  pla
  tax
  lda rc6
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
  cmp __sb
  bcc @s
@t:
  sbc __sb                      ; C is set
  inc __sa,x                    ; the quotient bit (bit 0 is clear after the shift)
@s:
  dey
  bne @bit
  dex
  bpl @byte
  sta __sr
  rts

udivmod32_fast:                      ; the speed variant: __sa / __sb -> __sa, __sa % __sb -> __sr (unsigned)
  lda __sb+1                    ; a divisor below 256 takes the byte loop
  ora __sb+2
  ora __sb+3
  bne udivmod32_wide
  lda __sb
  beq udivmod32_wide            ; a zero divisor takes the wide path: its quotient is all ones
  jmp udivmod32_fast_byte

udivmod32_wide:
  lda __sb                      ; a zero divisor: all-ones quotient, the dividend as remainder
  ora __sb+1
  ora __sb+2
  ora __sb+3
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
  cmp __sb,x
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
  lda __sb+3
  bne @align
  lda __sb+2
  cmp __st+3
  bne @cb
  lda __sb+1
  cmp __st+2
  bne @cb
  lda __sb
  cmp __st+1
  beq @bshift                   ; equal above, and the divisor's low byte is 0 after the shift
@cb:
  bcs @align
@bshift:
  lda __sb+2
  sta __sb+3
  lda __sb+1
  sta __sb+2
  lda __sb
  sta __sb+1
  lda #0
  sta __sb
  txa
  clc
  adc #8
  tax
  bne @abyte
@align:
  lda __sb+3                    ; the divisor above half the dividend: aligned
  cmp __st+3
  bne @cmph
  lda __sb+2
  cmp __st+2
  bne @cmph
  lda __sb+1
  cmp __st+1
  bne @cmph
  lda __sb
  cmp __st
  beq @shift
@cmph:
  bcs @aligned
@shift:
  asl __sb
  rol __sb+1
  rol __sb+2
  rol __sb+3
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
  cmp __sb+3
  bne @c2
  lda __sr+2
  cmp __sb+2
  bne @c2
  lda __sr+1
  cmp __sb+1
  bne @c2
  lda __sr
  cmp __sb
@c2:
  bcc @no                       ; the remainder is below the divisor: the quotient bit is 0 (C clear)
  lda __sr                      ; C is set: no borrow, and the quotient bit is 1
  sbc __sb
  sta __sr
  lda __sr+1
  sbc __sb+1
  sta __sr+1
  lda __sr+2
  sbc __sb+2
  sta __sr+2
  lda __sr+3
  sbc __sb+3
  sta __sr+3
@no:
  rol __sa                      ; the quotient bit comes in at the bottom
  rol __sa+1
  rol __sa+2
  rol __sa+3
  lsr __sb+3
  ror __sb+2
  ror __sb+1
  ror __sb
  dex
  bne @trip
  rts

.export __udivsi3_fast
__udivsi3_fast:                 ; __sa / __sb -> __sa
  jmp udivmod32_fast

.export __udivsi3_fast_byte
__udivsi3_fast_byte:            ; __sa / __sb -> __sa, a divisor of 1 to 255
  jmp udivmod32_fast_byte

.export __umodsi3_fast
__umodsi3_fast:                 ; __sa % __sb -> __sa
  jmp umod32_fast

.export __umodsi3_fast_byte
__umodsi3_fast_byte:            ; __sa % __sb -> __sa, a divisor of 1 to 255
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
  lda __sb+3
  bpl @b
  lda __sg
  eor #1
  sta __sg
  jsr neg_sb
@b:
  jmp udivmod32_fast

.export __divsi3_fast
__divsi3_fast:                  ; __sa / __sb -> __sa (signed, truncated toward zero)
  jsr divmod32s_fast
  lda __sg
  and #1
  beq @done
  jsr neg_sa
@done:
  rts

.export __modsi3_fast
__modsi3_fast:                  ; __sa % __sb -> __sa (signed, the sign of the dividend)
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

.export __divsi3_fast_byte
__divsi3_fast_byte:             ; __sa / __sb -> __sa (signed), a divisor of 1 to 255
  jsr divmod32s_fast_byte
  lda __sg
  and #1
  beq @done
  jsr neg_sa
@done:
  rts

.export __modsi3_fast_byte
__modsi3_fast_byte:             ; __sa % __sb -> __sa (signed), a divisor of 1 to 255
  jsr divmod32s_fast_byte
  jsr copy_sr_to_sa
  lda __sg
  and #2
  beq @done
  jsr neg_sa
@done:
  rts

; ---- the speed division with the small wide path: the routines of the module that divides cold
; (sites that run fewer than 3 times together, `hotdivision`) are the ones of the first speed variant,
; byte for byte: the pin-structure wide path above is 141 bytes more and pays only where it runs ----
udivmod32_fast_small:                      ; the speed variant: __sa / __sb -> __sa, __sa % __sb -> __sr (unsigned)
  lda #0
  sta __sr
  sta __sr+1
  sta __sr+2
  sta __sr+3
  lda __sb+1                    ; a divisor below 256: the remainder stays in A, a byte at a time
  ora __sb+2
  ora __sb+3
  bne @wide
  lda __sb
  beq @wide                     ; a zero divisor takes the general path: its quotient is all ones
  lda #0
  ldx #3
@byte:
  ldy #8
@bit:
  asl __sa,x
  rol a
  bcs @t                        ; the remainder overflowed 8 bits: it is above the divisor
  cmp __sb
  bcc @s
@t:
  sbc __sb                      ; C is set
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
  cmp __sb+3
  bne @below
  lda __sr+1
  cmp __sb+2
  bne @below
  lda __sr
  cmp __sb+1
  bne @below
  lda __sa+3
  cmp __sb
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
  cmp __sb+3
  bne @cmp
  lda __sr+2
  cmp __sb+2
  bne @cmp
  lda __sr+1
  cmp __sb+1
  bne @cmp
  lda __sr
  cmp __sb
@cmp:
  bcc @skipbit                  ; the remainder is below the divisor
@take:
  sec
  lda __sr
  sbc __sb
  sta __sr
  lda __sr+1
  sbc __sb+1
  sta __sr+1
  lda __sr+2
  sbc __sb+2
  sta __sr+2
  lda __sr+3
  sbc __sb+3
  sta __sr+3
  inc __sa                      ; the quotient bit (bit 0 of __sa is clear after the shift)
@skipbit:
  dex
  bne @next
  rts

.export __udivsi3_fast_small
__udivsi3_fast_small:                 ; __sa / __sb -> __sa
  jmp udivmod32_fast_small

.export __umodsi3_fast_small
__umodsi3_fast_small:                 ; __sa % __sb -> __sa
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
  lda __sb+3
  bpl @b
  lda __sg
  eor #1
  sta __sg
  jsr neg_sb
@b:
  jmp udivmod32_fast_small

.export __divsi3_fast_small
__divsi3_fast_small:                  ; __sa / __sb -> __sa (signed, truncated toward zero)
  jsr divmod32s_fast_small
  lda __sg
  and #1
  beq @done
  jsr neg_sa
@done:
  rts

.export __modsi3_fast_small
__modsi3_fast_small:                  ; __sa % __sb -> __sa (signed, the sign of the dividend)
  jsr divmod32s_fast_small
  jsr copy_sr_to_sa
  lda __sg
  and #2
  beq @done
  jsr neg_sa
@done:
  rts

; ---- the speed remainder: the quotient is never built, as in llvm-mos's `__umodsi3`. The dividend is
; the remainder, in place in __sa; the divisor is shifted up as far as the quotient's first bit, and each
; trip subtracts it when it fits and halves it ----
umod32_fast:                         ; __sa % __sb -> __sa (unsigned)
  lda __sb
  ora __sb+1
  ora __sb+2
  ora __sb+3
  beq @zero                     ; a zero divisor: the dividend is the remainder
  lda __sb+1                    ; a divisor below 256 takes the byte loop
  ora __sb+2
  ora __sb+3
  bne umod32_wide
  jsr udivmod32_fast_byte
  jmp copy_sr_to_sa
@zero:
  rts

umod32_wide:
  ldx #3                        ; a divisor above the dividend: the dividend is the remainder
@cmpd:
  lda __sa,x
  cmp __sb,x
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
  lda __sb+3
  bne @align
  lda __sb+2
  cmp __st+3
  bne @cb
  lda __sb+1
  cmp __st+2
  bne @cb
  lda __sb
  cmp __st+1
  beq @bshift                   ; equal above, and the divisor's low byte is 0 after the shift
@cb:
  bcs @align
@bshift:
  lda __sb+2
  sta __sb+3
  lda __sb+1
  sta __sb+2
  lda __sb
  sta __sb+1
  lda #0
  sta __sb
  txa
  clc
  adc #8
  tax
  bne @abyte
@align:
  lda __sb+3                    ; the divisor above half the dividend: aligned
  cmp __st+3
  bne @cmph
  lda __sb+2
  cmp __st+2
  bne @cmph
  lda __sb+1
  cmp __st+1
  bne @cmph
  lda __sb
  cmp __st
  beq @shift
@cmph:
  bcs @aligned
@shift:
  asl __sb
  rol __sb+1
  rol __sb+2
  rol __sb+3
  inx
  bne @align                    ; X is never 0 here: the loop is past a short branch's reach otherwise
@aligned:
  inx
@trip:
  lda __sa+3                    ; compare from the high byte: most trips end at the first
  cmp __sb+3
  bne @c2
  lda __sa+2
  cmp __sb+2
  bne @c2
  lda __sa+1
  cmp __sb+1
  bne @c2
  lda __sa
  cmp __sb
@c2:
  bcc @no                       ; the remainder is below the divisor: the quotient bit is 0 (C clear)
  lda __sa                      ; C is set: no borrow, and the quotient bit is 1
  sbc __sb
  sta __sa
  lda __sa+1
  sbc __sb+1
  sta __sa+1
  lda __sa+2
  sbc __sb+2
  sta __sa+2
  lda __sa+3
  sbc __sb+3
  sta __sa+3
@no:
  lsr __sb+3
  ror __sb+2
  ror __sb+1
  ror __sb
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
  lda __sb+3
  bpl @b
  jsr neg_sb
@b:
  jsr umod32_fast
  lda __sg
  beq @done
  jmp neg_sa
@done:
  rts
