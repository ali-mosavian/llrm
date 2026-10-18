declare i32 @llvm.ctpop.i32(i32)

define i32 @unsigned(i32 %x, i32 %y) {
entry:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %lt, label %ge
lt:
  %a = add i32 %x, 1
  ret i32 %a
ge:
  %d = icmp ugt i32 %x, 20
  br i1 %d, label %big, label %mid
big:
  %b = sub i32 %x, 20
  ret i32 %b
mid:
  %m = mul i32 %x, 3
  ret i32 %m
}

define i32 @signed(i32 %x) {
entry:
  %c = icmp slt i32 %x, -5
  br i1 %c, label %neg, label %rest
neg:
  %n = sub i32 0, %x
  ret i32 %n
rest:
  %d = icmp sle i32 %x, 100
  br i1 %d, label %mid, label %big
mid:
  %m = add i32 %x, 7
  ret i32 %m
big:
  %b = lshr i32 %x, 4
  ret i32 %b
}

define i32 @equality(i32 %x) {
entry:
  %c = icmp eq i32 %x, 42
  br i1 %c, label %is, label %isnt
is:
  %a = add i32 %x, 1
  ret i32 %a
isnt:
  %d = icmp ne i32 %x, 0
  br i1 %d, label %nz, label %z
nz:
  %b = udiv i32 100, %x
  ret i32 %b
z:
  ret i32 %x
}

define i8 @offset(i8 %x) {
entry:
  %t = add i8 %x, -10
  %c = icmp ult i8 %t, 5
  br i1 %c, label %in, label %out
in:
  %a = add i8 %x, 1
  ret i8 %a
out:
  ret i8 %x
}

define i8 @symmetric(i8 %x) {
entry:
  %p = add i8 %x, 1
  %c = icmp ult i8 %p, 8
  br i1 %c, label %in, label %out
in:
  %a = mul i8 %x, 2
  ret i8 %a
out:
  ret i8 %p
}

define i32 @or_and(i32 %x, i32 %y) {
entry:
  %o = or i32 %x, %y
  %c = icmp ult i32 %o, 100
  br i1 %c, label %a, label %b
a:
  %r = add i32 %x, %y
  ret i32 %r
b:
  %an = and i32 %x, %y
  %d = icmp ugt i32 %an, 200
  br i1 %d, label %c2, label %e
c2:
  %r2 = sub i32 %x, %y
  ret i32 %r2
e:
  ret i32 0
}

define i32 @urem_trunc(i32 %x, i32 %m) {
entry:
  %r = urem i32 %x, %m
  %c = icmp uge i32 %r, 50
  br i1 %c, label %big, label %small
big:
  %a = add i32 %x, 1
  ret i32 %a
small:
  %t = trunc i32 %x to i8
  %d = icmp uge i8 %t, 7
  br i1 %d, label %t2, label %t3
t2:
  %b = sub i32 %x, 1
  ret i32 %b
t3:
  ret i32 %x
}

define i32 @ashr_slt(i32 %x) {
entry:
  %s = ashr i32 %x, 4
  %c = icmp slt i32 %s, 10
  br i1 %c, label %a, label %b
a:
  %r = add i32 %x, 0
  ret i32 %r
b:
  %d = icmp sgt i32 %s, 100
  br i1 %d, label %c2, label %e
c2:
  %r2 = add i32 %x, 1
  ret i32 %r2
e:
  ret i32 %x
}

define i32 @mask(i32 %x) {
entry:
  %m = and i32 %x, 12
  %c = icmp eq i32 %m, 4
  br i1 %c, label %eq, label %ne
eq:
  %a = add i32 %x, 0
  ret i32 %a
ne:
  %d = icmp ne i32 %m, 0
  br i1 %d, label %nz, label %z
nz:
  %b = add i32 %x, 1
  ret i32 %b
z:
  ret i32 %x
}

define i32 @popcount(i32 %x) {
entry:
  %p = call i32 @llvm.ctpop.i32(i32 %x)
  %c = icmp ult i32 %p, 3
  br i1 %c, label %few, label %many
few:
  %a = add i32 %x, 0
  ret i32 %a
many:
  %d = icmp eq i32 %p, 32
  br i1 %d, label %all, label %some
all:
  %b = add i32 %x, 1
  ret i32 %b
some:
  ret i32 %x
}

define i32 @square(i32 %x) {
entry:
  %sq = mul nuw i32 %x, %x
  %c = icmp ult i32 %sq, 100
  br i1 %c, label %a, label %b
a:
  %r = add i32 %x, 0
  ret i32 %r
b:
  ret i32 %x
}

define i32 @sub_eq(i32 %a, i32 %b) {
entry:
  %s = sub i32 %a, %b
  %c = icmp eq i32 %a, %b
  br i1 %c, label %same, label %diff
same:
  %r = add i32 %s, 0
  ret i32 %r
diff:
  %d = add i32 %s, 1
  ret i32 %d
}

define i32 @swapped(i32 %x) {
entry:
  %c = icmp ugt i32 10, %x
  br i1 %c, label %a, label %b
a:
  %r = add i32 %x, 0
  ret i32 %r
b:
  %s = add i32 %x, 1
  ret i32 %s
}
