; RUN: llrm-llc -O2 -fsanitize=stack -S -o /dev/stdout %s
; -fsanitize=stack on LLVM IR (llrm-llc and llrm-cc ignored it; only llrm-c's HIR path checked): a function compares SP
; with the OS layer's LL$STACK_LOW once its frame is allocated and calls __STKOVERFLOW out of line, last.
; CHECK-LABEL: deep{{(@3)?}} proc
; CHECK: cmp sp, word ptr LL$STACK_LOW
; CHECK-NEXT: jb
; CHECK: call far ptr __STKOVERFLOW
; CHECK-NEXT: deep{{(@3)?}} endp
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define i16 @deep(i16 %n) {
entry:
  %pad = alloca [48 x i8], align 1
  %c = trunc i16 %n to i8
  store volatile i8 %c, ptr %pad, align 1
  %z = icmp eq i16 %n, 0
  br i1 %z, label %done, label %rec

rec:
  %m = add i16 %n, -1
  %r = call i16 @deep(i16 %m)
  %s = add i16 %r, 1
  ret i16 %s

done:
  ret i16 0
}
