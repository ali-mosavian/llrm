target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

; sumhi(n, ...): the sum of the high words of n variadic doubles.
define i32 @sumhi(i32 %n, ...) {
entry:
  %ap = alloca ptr, align 2
  call void @llvm.va_start.p0(ptr %ap)
  %empty = icmp eq i32 %n, 0
  br i1 %empty, label %done, label %loop

loop:
  %left = phi i32 [ %next, %loop ], [ %n, %entry ]
  %acc = phi i32 [ %sum, %loop ], [ 0, %entry ]
  %next = add i32 %left, -1
  %d = va_arg ptr %ap, double
  %bits = bitcast double %d to i64
  %shifted = lshr i64 %bits, 32
  %hi = trunc i64 %shifted to i32
  %sum = add i32 %acc, %hi
  %again = icmp eq i32 %next, 0
  br i1 %again, label %done, label %loop

done:
  %result = phi i32 [ 0, %entry ], [ %sum, %loop ]
  call void @llvm.va_end.p0(ptr %ap)
  ret i32 %result
}

declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)

define i32 @call_a(double %a, double %b, double %c) noinline {
  %r = call i32 (i32, ...) @sumhi(i32 3, double %a, double %b, double %c)
  ret i32 %r
}

; a volatile local moves the frame by a word, so sp is on the other residue mod 8
define i32 @call_b(double %a, double %b, double %c) noinline {
  %pad = alloca i32, align 2
  store volatile i32 1, ptr %pad, align 2
  %r = call i32 (i32, ...) @sumhi(i32 3, double %a, double %b, double %c)
  %p = load volatile i32, ptr %pad, align 2
  %q = add i32 %r, %p
  %s = add i32 %q, -1
  ret i32 %s
}
