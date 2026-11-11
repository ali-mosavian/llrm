target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @f(i32 %t, i32 %n) {
entry:
  %a = alloca [8 x i32]
  br label %fill
fill:
  %i = phi i32 [0, %entry], [%i1, %fill]
  %v = mul i32 %i, 5
  %w = and i32 %v, 7
  %p = getelementptr i32, ptr %a, i32 %i
  store i32 %w, ptr %p
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, 8
  br i1 %c, label %fill, label %count
count:
  %j = phi i32 [0, %fill], [%j1, %count]
  %s = phi i32 [0, %fill], [%s1, %count]
  %q = getelementptr i32, ptr %a, i32 %j
  %x = load i32, ptr %q
  %g = icmp sgt i32 %x, %t
  %z = zext i1 %g to i32
  %s1 = add i32 %s, %z
  %j1 = add i32 %j, 1
  %d = icmp slt i32 %j1, %n
  br i1 %d, label %count, label %done
done:
  ret i32 %s1
}
