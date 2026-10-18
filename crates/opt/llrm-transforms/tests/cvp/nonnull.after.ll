declare void @g(ptr)

define void @f(ptr %p) {
entry:
  %c = icmp eq ptr %p, null
  br i1 %c, label %out, label %then

then:
  call void @g(ptr nonnull %p)
  ret void

out:
  ret void
}
