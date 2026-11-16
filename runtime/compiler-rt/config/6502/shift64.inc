; 64-bit shifts for the 6502: u64 __ashldi3(u64 x, int n), __lshrdi3, __ashrdi3, llrm-6502 A:X-first layout (as di64.s): x0:x1 in A:X, x2..x7 in
; rc2..rc7, n at the caller's stack (rc0:rc1)+0 (read only), 0 <= n < 64 (the C shift's contract). Result A:X:rc2..rc7. Scratch rc16..rc25.
; The value goes to v (rc16..rc23), n / 8 whole bytes move (a trip a byte), then n % 8 bits shift through the eight bytes, one carry chain
; a trip: llvm-mos's routines are the bit loop alone (31 B) and take 8 times the bit trips for a count of 8 or more. The C members were 205,
; 221 and 230 B.

; The prologue and epilogue of the 64-bit shifts (shift64_*.s): the value in v, X = n / 8, cnt = n % 8; and v back in A:X:rc2..rc7.
rc0 = $80
rc2 = $82
v = $90                 ; rc16..rc23
cnt = $98               ; rc24: n % 8
fill = $99              ; rc25: the byte a right shift brings in


; v = x, A = n
.macro SHIFT64_LOAD
    sta v
    stx v+1
    ldx #5
:   lda rc2,x
    sta v+2,x
    dex
    bpl :-
    ldy #0
    lda (rc0),y
.endmacro

; A = n: X = n / 8 and cnt = n % 8 (v is already set: the _a entries are the fix_udi.s ones)
.macro SHIFT64_COUNT
    tay
    and #7
    sta cnt
    tya
    lsr a
    lsr a
    lsr a
    tax
.endmacro

; v = x, then X = n / 8 and cnt = n % 8
.macro SHIFT64_ENTER
    SHIFT64_LOAD
    SHIFT64_COUNT
.endmacro

; A:X:rc2..rc7 = v
.macro SHIFT64_LEAVE
    ldx #5
:   lda v+2,x
    sta rc2,x
    dex
    bpl :-
    lda v
    ldx v+1
    rts
.endmacro
