target triple = "wasm32-unknown-unknown"
declare void @g4()
declare void @g3()
declare void @g2()
declare void @g1()
declare void @g0()
declare void @f(i32)
define void @sw(ptr %p, i1 %c) {
entry:
  %v = load i8, ptr %p
  br i1 %c, label %b, label %out
b:
  switch i8 %v, label %out [ i8 -64, label %c0
                              i8 -63, label %c1
                              i8 -62, label %c2
                              i8 -61, label %c3
                              i8 -60, label %c4 ]
c0: call void @g0()
  br label %out
c1: call void @g1()
  br label %out
c2: call void @g2()
  br label %out
c3: call void @g3()
  br label %out
c4: call void @g4()
  br label %out
out:
  ret void
}
