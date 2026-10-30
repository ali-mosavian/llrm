define i32 @cases(i32 %x) {
entry:
  switch i32 %x, label %def [
    i32 1, label %one
    i32 2, label %one
    i32 5, label %five
    i32 9, label %def
  ]
one:
  %a = add i32 %x, 1
  ret i32 %a
five:
  %b = add i32 %x, 2
  ret i32 %b
def:
  %c = add i32 %x, 3
  ret i32 %c
}

define i32 @foldable(i8 %x) {
entry:
  %z = zext i8 %x to i32
  switch i8 %x, label %def [
    i8 3, label %a
    i8 7, label %b
  ]
a:
  %r = add i32 %z, 0
  ret i32 %r
b:
  %r2 = add i32 %z, 0
  ret i32 %r2
def:
  %r3 = add i32 %z, 0
  ret i32 %r3
}

define i32 @foldable_binop(i32 %x) {
entry:
  %m = mul i32 %x, 4
  switch i32 %x, label %def [
    i32 3, label %a
    i32 100, label %b
  ]
a:
  %r = add i32 %m, 0
  ret i32 %r
b:
  %r2 = add i32 %m, 0
  ret i32 %r2
def:
  ret i32 0
}

define i32 @default_only(i32 %x) {
entry:
  switch i32 %x, label %def [
    i32 0, label %a
  ]
a:
  ret i32 1
def:
  %r = udiv i32 100, %x
  ret i32 %r
}
