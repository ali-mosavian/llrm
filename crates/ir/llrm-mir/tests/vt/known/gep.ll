target datalayout = "e-p:64:64-p1:64:64:64:32-p2:32:32:32:16-i64:64"

%S = type { i8, i32, [3 x i16], i64 }
%P = type <{ i8, i32 }>
%O = type opaque

@g = global [16 x i32] zeroinitializer, align 8
@gs = global %S zeroinitializer
@gb = global i8 0

define ptr @arrays(ptr align 16 %p, i64 %i, i64 %j) {
  %a = getelementptr i32, ptr %p, i64 3
  %b = getelementptr i32, ptr %p, i64 %i
  %i4 = shl i64 %i, 2
  %c = getelementptr i8, ptr %p, i64 %i4
  %d = getelementptr [4 x i16], ptr %p, i64 %i, i64 %j
  %e = getelementptr [4 x i16], ptr %p, i64 1, i64 3
  %f = getelementptr i32, ptr %p, i64 0
  %g1 = getelementptr i64, ptr @g, i64 %i
  ret ptr %a
}

define ptr @structs(ptr align 8 %p, i64 %i) {
  %a = getelementptr %S, ptr %p, i64 0, i32 1
  %b = getelementptr %S, ptr %p, i64 0, i32 2, i64 2
  %c = getelementptr %S, ptr %p, i64 0, i32 3
  %d = getelementptr %S, ptr %p, i64 %i, i32 2
  %e = getelementptr %S, ptr %p, i64 1
  %f = getelementptr %P, ptr %p, i64 0, i32 1
  %g1 = getelementptr %P, ptr %p, i64 %i, i32 1
  %h = getelementptr %S, ptr @gs, i64 0, i32 3
  ret ptr %a
}

define ptr addrspace(1) @index_widths(ptr addrspace(1) align 8 %p, ptr addrspace(2) align 4 %q, i64 %i, i32 %j) {
  %a = getelementptr i32, ptr addrspace(1) %p, i64 %i
  %b = getelementptr i32, ptr addrspace(1) %p, i32 %j
  %c = getelementptr i32, ptr addrspace(2) %q, i64 %i
  %d = getelementptr i32, ptr addrspace(2) %q, i32 %j
  %e = getelementptr i8, ptr addrspace(2) %q, i32 -3
  %f = getelementptr i8, ptr addrspace(1) %p, i64 -300
  %n = or i32 %j, -2147483648
  %h = getelementptr i64, ptr addrspace(2) %q, i32 %n
  ret ptr addrspace(1) %a
}

define ptr @consts(i64 %i) {
  %a = getelementptr i8, ptr @gb, i64 5
  %b = getelementptr i8, ptr getelementptr (i8, ptr @g, i64 12), i64 %i
  %c = getelementptr i32, ptr @g, i64 2
  %d = getelementptr i8, ptr null, i64 24
  %e = getelementptr i8, ptr inttoptr (i64 4096 to ptr), i64 8
  %f = getelementptr i8, ptr inttoptr (i64 48 to ptr), i64 %i
  ret ptr %a
}

define i32 @ptr_int(ptr align 8 %p, i32 %x) {
  %a = ptrtoint ptr %p to i32
  %b = ptrtoint ptr %p to i64
  %c = inttoptr i32 %x to ptr
  %o = or i32 %x, 15
  %d = inttoptr i32 %o to ptr
  %e = ptrtoint ptr %d to i64
  ret i32 %a
}

define i64 @bitcasts(ptr align 4 %p, i64 %x) {
  %a = bitcast ptr %p to ptr
  %b = bitcast i64 %x to i64
  ret i64 %b
}
