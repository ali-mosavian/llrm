target datalayout = "@m64-datalayout@"
target triple = "x86_64-unknown-linux-gnu"

define dso_local i64 @f(i64 %x, i32 %c) {
  %t = icmp sgt i32 %c, 0
  %s = select i1 %t, i64 640, i64 -640
  %r = add i64 %s, %x
  ret i64 %r
}
