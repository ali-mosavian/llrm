target datalayout = "@m64-datalayout@"
target triple = "x86_64-unknown-linux-gnu"

@at = internal constant [18 x ptr] [ptr blockaddress(@run, %h0), ptr blockaddress(@run, %h1), ptr blockaddress(@run, %h2), ptr blockaddress(@run, %h3), ptr blockaddress(@run, %h4), ptr blockaddress(@run, %h5), ptr blockaddress(@run, %h6), ptr blockaddress(@run, %h7), ptr blockaddress(@run, %h8), ptr blockaddress(@run, %h9), ptr blockaddress(@run, %h10), ptr blockaddress(@run, %h11), ptr blockaddress(@run, %h12), ptr blockaddress(@run, %h13), ptr blockaddress(@run, %h14), ptr blockaddress(@run, %h15), ptr blockaddress(@run, %h16), ptr blockaddress(@run, %h17)], align 8
@count = dso_local global i64 0, align 8

declare i32 @fail(ptr)

define dso_local void @run(ptr noundef %v, ptr noundef %ip0) nounwind uwtable {
entry:
  br label %next

next:
  %ip = phi ptr [ %ip0, %entry ], [ %m0, %match0 ], [ %m1, %match1 ], [ %m2, %match2 ], [ %m3, %match3 ], [ %m4, %match4 ], [ %m5, %match5 ], [ %m6, %match6 ], [ %m7, %match7 ], [ %m8, %match8 ], [ %m9, %match9 ], [ %m10, %match10 ], [ %m11, %match11 ], [ %m12, %match12 ], [ %m13, %match13 ], [ %m14, %match14 ], [ %m15, %match15 ], [ %m16, %match16 ], [ %m17, %match17 ], [ %o0, %other0 ], [ %o1, %other1 ], [ %o2, %other2 ], [ %o3, %other3 ], [ %o4, %other4 ], [ %o5, %other5 ], [ %o6, %other6 ], [ %o7, %other7 ], [ %o8, %other8 ], [ %o9, %other9 ], [ %o10, %other10 ], [ %o11, %other11 ], [ %o12, %other12 ], [ %o13, %other13 ], [ %o14, %other14 ], [ %o15, %other15 ], [ %o16, %other16 ], [ %o17, %other17 ]
  %n = load i64, ptr @count, align 8
  %n1 = add i64 %n, 1
  store i64 %n1, ptr @count, align 8
  %op = load i8, ptr %ip, align 1
  %ix = zext i8 %op to i64
  %slot = getelementptr inbounds [18 x ptr], ptr @at, i64 0, i64 %ix
  %dst = load ptr, ptr %slot, align 8
  indirectbr ptr %dst, [label %h0, label %h1, label %h2, label %h3, label %h4, label %h5, label %h6, label %h7, label %h8, label %h9, label %h10, label %h11, label %h12, label %h13, label %h14, label %h15, label %h16, label %h17]

h0:
  %a0 = getelementptr i8, ptr %ip, i64 1
  %w0 = load i32, ptr %a0, align 1
  %c0 = icmp eq i32 %w0, 1
  br i1 %c0, label %match0, label %other0

match0:
  %b0 = getelementptr i8, ptr %ip, i64 5
  %d0 = load i32, ptr %b0, align 1
  %e0 = sext i32 %d0 to i64
  %m0 = getelementptr i8, ptr %ip, i64 %e0
  br label %next

other0:
  %r0 = call i32 @fail(ptr %v)
  %o0 = getelementptr i8, ptr %ip, i64 5
  br label %next

h1:
  %a1 = getelementptr i8, ptr %ip, i64 1
  %w1 = load i32, ptr %a1, align 1
  %c1 = icmp eq i32 %w1, 2
  br i1 %c1, label %match1, label %other1

match1:
  %b1 = getelementptr i8, ptr %ip, i64 5
  %d1 = load i32, ptr %b1, align 1
  %e1 = sext i32 %d1 to i64
  %m1 = getelementptr i8, ptr %ip, i64 %e1
  br label %next

other1:
  %r1 = call i32 @fail(ptr %v)
  %o1 = getelementptr i8, ptr %ip, i64 6
  br label %next

h2:
  %a2 = getelementptr i8, ptr %ip, i64 1
  %w2 = load i32, ptr %a2, align 1
  %c2 = icmp eq i32 %w2, 3
  br i1 %c2, label %match2, label %other2

match2:
  %b2 = getelementptr i8, ptr %ip, i64 5
  %d2 = load i32, ptr %b2, align 1
  %e2 = sext i32 %d2 to i64
  %m2 = getelementptr i8, ptr %ip, i64 %e2
  br label %next

other2:
  %r2 = call i32 @fail(ptr %v)
  %o2 = getelementptr i8, ptr %ip, i64 7
  br label %next

h3:
  %a3 = getelementptr i8, ptr %ip, i64 1
  %w3 = load i32, ptr %a3, align 1
  %c3 = icmp eq i32 %w3, 4
  br i1 %c3, label %match3, label %other3

match3:
  %b3 = getelementptr i8, ptr %ip, i64 5
  %d3 = load i32, ptr %b3, align 1
  %e3 = sext i32 %d3 to i64
  %m3 = getelementptr i8, ptr %ip, i64 %e3
  br label %next

other3:
  %r3 = call i32 @fail(ptr %v)
  %o3 = getelementptr i8, ptr %ip, i64 8
  br label %next

h4:
  %a4 = getelementptr i8, ptr %ip, i64 1
  %w4 = load i32, ptr %a4, align 1
  %c4 = icmp eq i32 %w4, 5
  br i1 %c4, label %match4, label %other4

match4:
  %b4 = getelementptr i8, ptr %ip, i64 5
  %d4 = load i32, ptr %b4, align 1
  %e4 = sext i32 %d4 to i64
  %m4 = getelementptr i8, ptr %ip, i64 %e4
  br label %next

other4:
  %r4 = call i32 @fail(ptr %v)
  %o4 = getelementptr i8, ptr %ip, i64 9
  br label %next

h5:
  %a5 = getelementptr i8, ptr %ip, i64 1
  %w5 = load i32, ptr %a5, align 1
  %c5 = icmp eq i32 %w5, 6
  br i1 %c5, label %match5, label %other5

match5:
  %b5 = getelementptr i8, ptr %ip, i64 5
  %d5 = load i32, ptr %b5, align 1
  %e5 = sext i32 %d5 to i64
  %m5 = getelementptr i8, ptr %ip, i64 %e5
  br label %next

other5:
  %r5 = call i32 @fail(ptr %v)
  %o5 = getelementptr i8, ptr %ip, i64 10
  br label %next

h6:
  %a6 = getelementptr i8, ptr %ip, i64 1
  %w6 = load i32, ptr %a6, align 1
  %c6 = icmp eq i32 %w6, 7
  br i1 %c6, label %match6, label %other6

match6:
  %b6 = getelementptr i8, ptr %ip, i64 5
  %d6 = load i32, ptr %b6, align 1
  %e6 = sext i32 %d6 to i64
  %m6 = getelementptr i8, ptr %ip, i64 %e6
  br label %next

other6:
  %r6 = call i32 @fail(ptr %v)
  %o6 = getelementptr i8, ptr %ip, i64 11
  br label %next

h7:
  %a7 = getelementptr i8, ptr %ip, i64 1
  %w7 = load i32, ptr %a7, align 1
  %c7 = icmp eq i32 %w7, 8
  br i1 %c7, label %match7, label %other7

match7:
  %b7 = getelementptr i8, ptr %ip, i64 5
  %d7 = load i32, ptr %b7, align 1
  %e7 = sext i32 %d7 to i64
  %m7 = getelementptr i8, ptr %ip, i64 %e7
  br label %next

other7:
  %r7 = call i32 @fail(ptr %v)
  %o7 = getelementptr i8, ptr %ip, i64 12
  br label %next

h8:
  %a8 = getelementptr i8, ptr %ip, i64 1
  %w8 = load i32, ptr %a8, align 1
  %c8 = icmp eq i32 %w8, 9
  br i1 %c8, label %match8, label %other8

match8:
  %b8 = getelementptr i8, ptr %ip, i64 5
  %d8 = load i32, ptr %b8, align 1
  %e8 = sext i32 %d8 to i64
  %m8 = getelementptr i8, ptr %ip, i64 %e8
  br label %next

other8:
  %r8 = call i32 @fail(ptr %v)
  %o8 = getelementptr i8, ptr %ip, i64 13
  br label %next

h9:
  %a9 = getelementptr i8, ptr %ip, i64 1
  %w9 = load i32, ptr %a9, align 1
  %c9 = icmp eq i32 %w9, 10
  br i1 %c9, label %match9, label %other9

match9:
  %b9 = getelementptr i8, ptr %ip, i64 5
  %d9 = load i32, ptr %b9, align 1
  %e9 = sext i32 %d9 to i64
  %m9 = getelementptr i8, ptr %ip, i64 %e9
  br label %next

other9:
  %r9 = call i32 @fail(ptr %v)
  %o9 = getelementptr i8, ptr %ip, i64 14
  br label %next

h10:
  %a10 = getelementptr i8, ptr %ip, i64 1
  %w10 = load i32, ptr %a10, align 1
  %c10 = icmp eq i32 %w10, 11
  br i1 %c10, label %match10, label %other10

match10:
  %b10 = getelementptr i8, ptr %ip, i64 5
  %d10 = load i32, ptr %b10, align 1
  %e10 = sext i32 %d10 to i64
  %m10 = getelementptr i8, ptr %ip, i64 %e10
  br label %next

other10:
  %r10 = call i32 @fail(ptr %v)
  %o10 = getelementptr i8, ptr %ip, i64 15
  br label %next

h11:
  %a11 = getelementptr i8, ptr %ip, i64 1
  %w11 = load i32, ptr %a11, align 1
  %c11 = icmp eq i32 %w11, 12
  br i1 %c11, label %match11, label %other11

match11:
  %b11 = getelementptr i8, ptr %ip, i64 5
  %d11 = load i32, ptr %b11, align 1
  %e11 = sext i32 %d11 to i64
  %m11 = getelementptr i8, ptr %ip, i64 %e11
  br label %next

other11:
  %r11 = call i32 @fail(ptr %v)
  %o11 = getelementptr i8, ptr %ip, i64 16
  br label %next

h12:
  %a12 = getelementptr i8, ptr %ip, i64 1
  %w12 = load i32, ptr %a12, align 1
  %c12 = icmp eq i32 %w12, 13
  br i1 %c12, label %match12, label %other12

match12:
  %b12 = getelementptr i8, ptr %ip, i64 5
  %d12 = load i32, ptr %b12, align 1
  %e12 = sext i32 %d12 to i64
  %m12 = getelementptr i8, ptr %ip, i64 %e12
  br label %next

other12:
  %r12 = call i32 @fail(ptr %v)
  %o12 = getelementptr i8, ptr %ip, i64 17
  br label %next

h13:
  %a13 = getelementptr i8, ptr %ip, i64 1
  %w13 = load i32, ptr %a13, align 1
  %c13 = icmp eq i32 %w13, 14
  br i1 %c13, label %match13, label %other13

match13:
  %b13 = getelementptr i8, ptr %ip, i64 5
  %d13 = load i32, ptr %b13, align 1
  %e13 = sext i32 %d13 to i64
  %m13 = getelementptr i8, ptr %ip, i64 %e13
  br label %next

other13:
  %r13 = call i32 @fail(ptr %v)
  %o13 = getelementptr i8, ptr %ip, i64 18
  br label %next

h14:
  %a14 = getelementptr i8, ptr %ip, i64 1
  %w14 = load i32, ptr %a14, align 1
  %c14 = icmp eq i32 %w14, 15
  br i1 %c14, label %match14, label %other14

match14:
  %b14 = getelementptr i8, ptr %ip, i64 5
  %d14 = load i32, ptr %b14, align 1
  %e14 = sext i32 %d14 to i64
  %m14 = getelementptr i8, ptr %ip, i64 %e14
  br label %next

other14:
  %r14 = call i32 @fail(ptr %v)
  %o14 = getelementptr i8, ptr %ip, i64 19
  br label %next

h15:
  %a15 = getelementptr i8, ptr %ip, i64 1
  %w15 = load i32, ptr %a15, align 1
  %c15 = icmp eq i32 %w15, 16
  br i1 %c15, label %match15, label %other15

match15:
  %b15 = getelementptr i8, ptr %ip, i64 5
  %d15 = load i32, ptr %b15, align 1
  %e15 = sext i32 %d15 to i64
  %m15 = getelementptr i8, ptr %ip, i64 %e15
  br label %next

other15:
  %r15 = call i32 @fail(ptr %v)
  %o15 = getelementptr i8, ptr %ip, i64 20
  br label %next

h16:
  %a16 = getelementptr i8, ptr %ip, i64 1
  %w16 = load i32, ptr %a16, align 1
  %c16 = icmp eq i32 %w16, 17
  br i1 %c16, label %match16, label %other16

match16:
  %b16 = getelementptr i8, ptr %ip, i64 5
  %d16 = load i32, ptr %b16, align 1
  %e16 = sext i32 %d16 to i64
  %m16 = getelementptr i8, ptr %ip, i64 %e16
  br label %next

other16:
  %r16 = call i32 @fail(ptr %v)
  %o16 = getelementptr i8, ptr %ip, i64 21
  br label %next

h17:
  %a17 = getelementptr i8, ptr %ip, i64 1
  %w17 = load i32, ptr %a17, align 1
  %c17 = icmp eq i32 %w17, 18
  br i1 %c17, label %match17, label %other17

match17:
  %b17 = getelementptr i8, ptr %ip, i64 5
  %d17 = load i32, ptr %b17, align 1
  %e17 = sext i32 %d17 to i64
  %m17 = getelementptr i8, ptr %ip, i64 %e17
  br label %next

other17:
  %r17 = call i32 @fail(ptr %v)
  %o17 = getelementptr i8, ptr %ip, i64 22
  br label %next

}
