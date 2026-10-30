target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx27.2.0"

define zeroext i8 @through(ptr %f) {
  %r = tail call zeroext i8 %f()
  ret i8 %r
}

define signext i16 @through_signed(ptr %f) {
  %r = tail call signext i16 %f()
  ret i16 %r
}
