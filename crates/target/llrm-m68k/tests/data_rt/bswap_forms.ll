target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

declare i32 @llvm.bswap.i32(i32)
declare i32 @llvm.bitreverse.i32(i32)

define i32 @bs(i32 %x) {
  %r = call i32 @llvm.bswap.i32(i32 %x)
  ret i32 %r
}

; one demanded byte: the swap goes, a shift by a constant is left
define i32 @bs_low(i32 %x) {
  %r = call i32 @llvm.bswap.i32(i32 %x)
  %m = and i32 %r, 255
  ret i32 %m
}

define i32 @bs_high(i32 %x) {
  %r = call i32 @llvm.bswap.i32(i32 %x)
  %m = and i32 %r, -16777216
  ret i32 %m
}

define i32 @rev(i32 %x) {
  %r = call i32 @llvm.bitreverse.i32(i32 %x)
  ret i32 %r
}

; the low byte of a bit reverse inside a loop: a combine made the shifts after legalization
define i32 @rev_loop(i32 %n) {
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %s = phi i32 [ 2463534242, %entry ], [ %s2, %loop ]
  %sum = phi i32 [ 0, %entry ], [ %sum2, %loop ]
  %a1 = shl i32 %s, 13
  %b1 = xor i32 %s, %a1
  %a2 = lshr i32 %b1, 17
  %b2 = xor i32 %b1, %a2
  %a3 = shl i32 %b2, 5
  %s2 = xor i32 %b2, %a3
  %r = call i32 @llvm.bitreverse.i32(i32 %s2)
  %low = and i32 %r, 255
  %sum2 = add i32 %sum, %low
  %next = add i32 %i, 1
  %done = icmp eq i32 %next, %n
  br i1 %done, label %exit, label %loop

exit:
  ret i32 %sum2
}
