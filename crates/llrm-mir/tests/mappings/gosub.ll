; GOSUB/RETURN: a continuation index on a stack, and a switch over the
; places control returns to. The subroutine adds 10 to total; it is
; reached from two sites, one nested.
; expect: 21
target datalayout = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16"

define i32 @main() {
entry:
  %stack = alloca [8 x i16], align 2
  %depth = alloca i16, align 2
  %total = alloca i16, align 2
  store i16 0, ptr %depth, align 2
  store i16 1, ptr %total, align 2
  call void @push(ptr %stack, ptr %depth, i16 0)
  br label %sub
after0:
  call void @push(ptr %stack, ptr %depth, i16 1)
  br label %sub
after1:
  %t = load i16, ptr %total, align 2
  %w = zext i16 %t to i32
  ret i32 %w
sub:
  %old = load i16, ptr %total, align 2
  %new = add i16 %old, 10
  store i16 %new, ptr %total, align 2
  %k = call i16 @pop(ptr %stack, ptr %depth)
  switch i16 %k, label %lost [ i16 0, label %after0
                               i16 1, label %after1 ]
lost:
  unreachable
}

define void @push(ptr %stack, ptr %depth, i16 %k) {
  %d = load i16, ptr %depth, align 2
  %slot = getelementptr inbounds [8 x i16], ptr %stack, i16 0, i16 %d
  store i16 %k, ptr %slot, align 2
  %next = add i16 %d, 1
  store i16 %next, ptr %depth, align 2
  ret void
}

define i16 @pop(ptr %stack, ptr %depth) {
  %d = load i16, ptr %depth, align 2
  %top = sub i16 %d, 1
  store i16 %top, ptr %depth, align 2
  %slot = getelementptr inbounds [8 x i16], ptr %stack, i16 0, i16 %top
  %k = load i16, ptr %slot, align 2
  ret i16 %k
}
