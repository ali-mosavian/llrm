; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128"

define void @test1(ptr noalias noundef %a, ptr noalias noundef %b, ptr noalias noundef %c) {
entry:
  br label %loop1.header

loop1.header:                                     ; preds = %loop1.latch, %entry
  %i2 = phi i64 [ 1, %entry ], [ %i2.inc, %loop1.latch ]
  %i2.st = add i64 %i2, 1
  %i2.ld = add i64 %i2, 0
  br label %loop2.header

loop2.header:                                     ; preds = %loop2.header, %loop1.header
  %i1 = phi i64 [ 1, %loop1.header ], [ %i1.inc, %loop2.header ]
  %i1.st = add i64 %i1, 0
  %i1.ld = add i64 %i1, 0
  %a.st = getelementptr inbounds [64 x i32], ptr %a, i64 %i1.st, i64 %i2.st
  %a.ld = getelementptr inbounds [64 x i32], ptr %a, i64 %i1.ld, i64 %i2.ld
  %b.ld = getelementptr inbounds [64 x i32], ptr %b, i64 %i1, i64 %i2
  %c.st = getelementptr inbounds [64 x i32], ptr %c, i64 %i1, i64 %i2
  %b.val = load i32, ptr %b.ld, align 4
  store i32 %b.val, ptr %a.st, align 4
  %a.val = load i32, ptr %a.ld, align 4
  store i32 %a.val, ptr %c.st, align 4
  %i1.inc = add nuw nsw i64 %i1, 1
  %loop2.exitcond.not = icmp eq i64 %i1.inc, 63
  br i1 %loop2.exitcond.not, label %loop1.latch, label %loop2.header

loop1.latch:                                      ; preds = %loop2.header
  %i2.inc = add nuw nsw i64 %i2, 1
  %loop1.exitcond.not = icmp eq i64 %i2.inc, 63
  br i1 %loop1.exitcond.not, label %exit, label %loop1.header

exit:                                             ; preds = %loop1.latch
  ret void
}

define void @test2(ptr noalias noundef %a, ptr noalias noundef %b, ptr noalias noundef %c) {
entry:
  br label %loop1.header

loop1.header:                                     ; preds = %loop1.latch, %entry
  %i2 = phi i64 [ 1, %entry ], [ %i2.inc, %loop1.latch ]
  %i2.st = add i64 %i2, 1
  %i2.ld = add i64 %i2, 0
  br label %loop2.header

loop2.header:                                     ; preds = %loop2.header, %loop1.header
  %i1 = phi i64 [ 1, %loop1.header ], [ %i1.inc, %loop2.header ]
  %i1.st = add i64 %i1, 0
  %i1.ld = add i64 %i1, 0
  %a.st = getelementptr inbounds [64 x i32], ptr %a, i64 %i1.st, i64 %i2.st
  %a.ld = getelementptr inbounds [64 x i32], ptr %a, i64 %i1.ld, i64 %i2.ld
  %b.ld = getelementptr inbounds [64 x i32], ptr %b, i64 %i1, i64 %i2
  %c.st = getelementptr inbounds [64 x i32], ptr %c, i64 %i1, i64 %i2
  %b.val = load i32, ptr %b.ld, align 4
  %a.val = load i32, ptr %a.ld, align 4
  store i32 %b.val, ptr %a.st, align 4
  store i32 %a.val, ptr %c.st, align 4
  %i1.inc = add nuw nsw i64 %i1, 1
  %loop2.exitcond.not = icmp eq i64 %i1.inc, 63
  br i1 %loop2.exitcond.not, label %loop1.latch, label %loop2.header

loop1.latch:                                      ; preds = %loop2.header
  %i2.inc = add nuw nsw i64 %i2, 1
  %loop1.exitcond.not = icmp eq i64 %i2.inc, 63
  br i1 %loop1.exitcond.not, label %exit, label %loop1.header

exit:                                             ; preds = %loop1.latch
  ret void
}
