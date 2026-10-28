target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

declare { i64, i1 } @step(i64)

define i64 @retry(i64 %start) {
entry:
  %first = call { i64, i1 } @step(i64 %start)
  br label %loop
loop:
  %acc = phi { i64, i1 } [ %first, %entry ], [ %next, %loop ]
  %v = extractvalue { i64, i1 } %acc, 0
  %ok = extractvalue { i64, i1 } %acc, 1
  %next = call { i64, i1 } @step(i64 %v)
  br i1 %ok, label %done, label %loop
done:
  ret i64 %v
}
