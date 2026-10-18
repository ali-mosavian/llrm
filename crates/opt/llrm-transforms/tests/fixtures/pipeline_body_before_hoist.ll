@_noise = internal constant [6 x i8] c"\0C\00\0D\00o\00"
@_mon_facts = external addrspace(1) constant [0 x i8]

declare cc 1005 void @"_ent_say@3"(ptr nocapture writeonly, ptr nocapture readnone, ptr addrspace(1)) addrspace(1) memory(argmem: readwrite) norecurse

declare cc 1005 void @"_ent_msg_frame@3"(ptr, float) addrspace(1)

declare cc 1005 void @"_ent_talk@3"(ptr, ptr, ptr nocapture writeonly, ptr nocapture readnone, ptr addrspace(1)) addrspace(1)

declare cc 1005 void @"_ent_kill_targets@3"(ptr nocapture, i16) memory(readwrite, argmem: read, inaccessiblemem: none) norecurse

declare cc 1005 i16 @"_ent_lightning_point@3"(ptr nocapture readonly, i16, ptr nocapture)

declare cc 1005 i16 @"_ent_lightning_points@3"(ptr nocapture readonly, ptr addrspace(1) nocapture readonly, ptr nocapture, ptr nocapture)

declare cc 1005 void @"_ent_lightning_beam@3"(ptr nocapture readonly, ptr, ptr addrspace(1) nocapture readonly, i16)

declare cc 1005 void @"_ent_lightning@3"(ptr, ptr, ptr, i16)

declare cc 1005 void @"_ent_use_targets@3"(ptr, ptr, ptr, ptr, i16) addrspace(1)

declare cc 1005 void @"_ent_trig_fire@3"(ptr, ptr, ptr, ptr, i16) addrspace(1)

declare cc 1005 i16 @"_ent_shot_button@3"(ptr addrspace(1) nocapture) memory(argmem: readwrite) willreturn norecurse

declare cc 1005 void @"_ent_button_frame@3"(ptr addrspace(1) nocapture, i16) memory(argmem: readwrite) willreturn norecurse

declare cc 1005 void @"_ent_button_return@3"(ptr addrspace(1) nocapture writeonly initializes((10, 12)), ptr addrspace(1) nocapture) memory(argmem: readwrite) willreturn norecurse

declare cc 1005 i16 @"_ent_trig_selftest@3"() addrspace(1) memory(readwrite, argmem: none)

declare cc 1005 void @"_ent_trig_shot@3"(ptr, ptr, ptr, ptr, i16) addrspace(1)

declare cc 1005 void @"_ent_move_trigs@3"(ptr, ptr, ptr, ptr, ptr, float, i16, i16, i16) addrspace(1)

declare cc 1005 void @"_ent_fireball_spawn@3"(ptr, ptr addrspace(1) nocapture writeonly) addrspace(1)

declare cc 1005 void @"_ent_monster_jump@3"(ptr, ptr addrspace(1)) addrspace(1)

declare cc 1005 i16 @"_ent_fireballs_sent@3"(ptr nocapture readonly) addrspace(1) memory(read, inaccessiblemem: none) norecurse

declare cc 1005 i16 @"_center_advance@3"(ptr, float) addrspace(1)

declare cc 1005 i16 @"_snd_self@3"(ptr, ptr, i16, i16) addrspace(1)

declare ptr addrspace(1) @__fmemcpy(ptr addrspace(1), ptr addrspace(1), i16) addrspace(1)

declare double @_sqrt(double) addrspace(1)

declare cc 1005 i16 @"_d_beam_spawn@3"(ptr, i16, i16, ptr, ptr) addrspace(1)

declare cc 1005 i16 @"_snd_start@3"(ptr, ptr, i16, i16, i16, ptr, i16) addrspace(1)

declare cc 1005 i16 @"_ent_door_fire@3"(ptr, ptr, ptr, ptr, i16) addrspace(1)

declare cc 1005 i16 @"_ent_lights_use@3"(ptr addrspace(1), i16, ptr, i16) addrspace(1)

declare cc 1005 i16 @"_ent_monster_teleport@3"(ptr, ptr, float, ptr addrspace(1)) addrspace(1)

declare cc 1005 i16 @"_part_lava@3"(ptr, ptr) addrspace(1)

declare cc 1005 ptr addrspace(1) @"_ent_msg@3"(ptr, i16) addrspace(1)

declare ptr @_memset(ptr, i16, i16) addrspace(1)

declare double @_atan2(double, double) addrspace(1)

declare cc 1005 i16 @"_mdl_boss_lava@3"(ptr, ptr, ptr, ptr, float, i16) addrspace(1)

declare cc 1005 i16 @"_ent_step_to@3"(ptr addrspace(1), ptr addrspace(1), float) addrspace(1)

declare cc 1005 i16 @"_ent_box_touched@3"(ptr, ptr addrspace(1), ptr addrspace(1), float) addrspace(1)

declare cc 1005 i16 @"_snd_play@3"(ptr, ptr, i16, ptr) addrspace(1)

declare cc 1005 i16 @"_ent_copy_map@3"(ptr, ptr addrspace(1)) addrspace(1)

declare cc 1005 i16 @"_pl_intermission@3"(ptr, ptr, ptr, i16, i16) addrspace(1)

declare cc 1005 i16 @"_ent_door_close@3"(ptr, ptr, i16) addrspace(1)

declare cc 1005 float @"_rng_float@3"(ptr) addrspace(1)

declare cc 1005 i16 @"_mdl_set_count@3"(ptr, i16) addrspace(1)

declare cc 1005 i16 @"_mdl_anim_start@3"(ptr addrspace(1), i16, i16) addrspace(1)

define internal cc 1005 void @pipeline.body(ptr %0, ptr %1, ptr %2, ptr %3, i16 %4) {
b1:
  %5 = alloca ptr
  %6 = alloca ptr
  %7 = alloca ptr
  %8 = alloca ptr
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca [12 x i8]
  store ptr %0, ptr %5, !tbaa !21
  store ptr %1, ptr %6, !tbaa !21
  store ptr %2, ptr %7, !tbaa !21
  store ptr %3, ptr %8, !tbaa !21
  store i16 %4, ptr %9, !tbaa !9
  store i16 0, ptr %11, !tbaa !9
  %13 = icmp eq i16 %4, 0
  br i1 %13, label %b4, label %b2

b2:
  store i16 0, ptr %10, !tbaa !9
  br label %b5

b4:
  ret void

b5:
  %14 = phi i16 [ 0, %b2 ], [ %25, %b8 ]
  %15 = getelementptr inbounds i8, ptr %0, i16 218
  %16 = load i16, ptr %15, !tbaa !9
  %17 = icmp slt i16 %14, %16
  br i1 %17, label %b7, label %b6

b6:
  store i16 0, ptr %10, !tbaa !9
  br label %b10

b7:
  %18 = getelementptr inbounds i8, ptr %0, i16 220
  %19 = load ptr addrspace(1), ptr %18, !tbaa !23
  %20 = mul i16 %14, 104
  %21 = getelementptr inbounds i8, ptr addrspace(1) %19, i16 %20
  %22 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 64
  %23 = load i16, ptr addrspace(1) %22, !tbaa !9
  %24 = icmp eq i16 %23, %4
  br i1 %24, label %b9, label %b8

b8:
  %25 = add nsw i16 %14, 1
  store i16 %25, ptr %10, !tbaa !9
  br label %b5

b9:
  %26 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 60
  %27 = load i16, ptr addrspace(1) %26, !tbaa !9
  %28 = call cc 1005 addrspace(1) i16 @"_ent_door_fire@3"(ptr %0, ptr %1, ptr %2, ptr %3, i16 %27)
  br label %b8

b10:
  %29 = phi i16 [ 0, %b6 ], [ %46, %b13 ]
  %30 = getelementptr inbounds i8, ptr %0, i16 210
  %31 = load i16, ptr %30, !tbaa !9
  %32 = icmp slt i16 %29, %31
  br i1 %32, label %b12, label %b11

b11:
  %33 = getelementptr inbounds i8, ptr %0, i16 238
  %34 = getelementptr inbounds i8, ptr %0, i16 236
  %35 = load i16, ptr %34, !tbaa !9
  %36 = getelementptr inbounds i8, ptr %0, i16 240
  %37 = load ptr addrspace(1), ptr %36, !tbaa !23
  %38 = call cc 1005 addrspace(1) i16 @"_ent_lights_use@3"(ptr addrspace(1) %37, i16 %35, ptr %33, i16 %4)
  store i16 0, ptr %10, !tbaa !9
  br label %b17

b12:
  %39 = getelementptr inbounds i8, ptr %0, i16 214
  %40 = load ptr addrspace(1), ptr %39, !tbaa !23
  %41 = mul i16 %29, 52
  %42 = getelementptr inbounds i8, ptr addrspace(1) %40, i16 %41
  %43 = getelementptr inbounds i8, ptr addrspace(1) %42, i16 36
  %44 = load i16, ptr addrspace(1) %43, !tbaa !9
  %45 = icmp eq i16 %44, 1
  br i1 %45, label %b14, label %b13

b13:
  %46 = add nsw i16 %29, 1
  store i16 %46, ptr %10, !tbaa !9
  br label %b10

b14:
  %47 = getelementptr inbounds i8, ptr addrspace(1) %42, i16 38
  %48 = load i16, ptr addrspace(1) %47, !tbaa !9
  %49 = icmp eq i16 %48, %4
  br i1 %49, label %b15, label %b13

b15:
  %50 = getelementptr inbounds i8, ptr addrspace(1) %42, i16 10
  %51 = load i16, ptr addrspace(1) %50, !tbaa !9
  %52 = icmp eq i16 %51, 2
  br i1 %52, label %b16, label %b13

b16:
  store i16 3, ptr addrspace(1) %50, !tbaa !9
  br label %b13

b17:
  %53 = phi i16 [ 0, %b11 ], [ %68, %b20 ]
  %54 = phi i16 [ 0, %b11 ], [ %67, %b20 ]
  %55 = getelementptr inbounds i8, ptr %0, i16 72
  %56 = load i16, ptr %55, !tbaa !9
  %57 = icmp slt i16 %53, %56
  br i1 %57, label %b19, label %b18

b18:
  %58 = phi i16 [ %54, %b17 ]
  %59 = icmp ne i16 %58, 0
  br i1 %59, label %b23, label %b22

b19:
  %60 = getelementptr inbounds i8, ptr %0, i16 74
  %61 = load ptr addrspace(1), ptr %60, !tbaa !23
  %62 = mul i16 %53, 48
  %63 = getelementptr inbounds i8, ptr addrspace(1) %61, i16 %62
  %64 = getelementptr inbounds i8, ptr addrspace(1) %63, i16 40
  %65 = load i16, ptr addrspace(1) %64, !tbaa !9
  %66 = icmp eq i16 %65, %4
  br i1 %66, label %b21, label %b20

b20:
  %67 = phi i16 [ %54, %b19 ], [ -1, %b21 ]
  %68 = add nsw i16 %53, 1
  store i16 %68, ptr %10, !tbaa !9
  br label %b17

b21:
  %69 = getelementptr inbounds i8, ptr %3, i16 14
  %70 = load float, ptr %69, !tbaa !15
  %71 = fadd float %70, 0x3FC99999A0000000
  %72 = getelementptr inbounds i8, ptr addrspace(1) %63, i16 44
  store float %71, ptr addrspace(1) %72, !tbaa !15
  store i16 -1, ptr %11, !tbaa !9
  br label %b20

b22:
  store i16 0, ptr %10, !tbaa !9
  br label %b27

b23:
  store i16 0, ptr %10, !tbaa !9
  br label %b24

b24:
  %73 = phi i16 [ 0, %b23 ], [ %84, %b26 ]
  %74 = getelementptr inbounds i8, ptr %0, i16 294
  %75 = load i16, ptr %74, !tbaa !9
  %76 = icmp slt i16 %73, %75
  br i1 %76, label %b26, label %b25

b25:
  br label %b22

b26:
  %77 = getelementptr inbounds i8, ptr %0, i16 296
  %78 = load ptr addrspace(1), ptr %77, !tbaa !23
  %79 = mul i16 %73, 104
  %80 = getelementptr inbounds i8, ptr addrspace(1) %78, i16 %79
  %81 = getelementptr inbounds i8, ptr %3, i16 14
  %82 = load float, ptr %81, !tbaa !15
  %83 = call cc 1005 addrspace(1) i16 @"_ent_monster_teleport@3"(ptr %0, ptr %3, float %82, ptr addrspace(1) %80)
  %84 = add nsw i16 %73, 1
  store i16 %84, ptr %10, !tbaa !9
  br label %b24

b27:
  %85 = phi i16 [ 0, %b22 ], [ %98, %b32 ]
  %86 = getelementptr inbounds i8, ptr %0, i16 224
  %87 = load i16, ptr %86, !tbaa !9
  %88 = icmp slt i16 %85, %87
  br i1 %88, label %b29, label %b28

b28:
  br label %b4

b29:
  %89 = getelementptr inbounds i8, ptr %0, i16 226
  %90 = load ptr addrspace(1), ptr %89, !tbaa !23
  %91 = mul i16 %85, 78
  %92 = getelementptr inbounds i8, ptr addrspace(1) %90, i16 %91
  %93 = getelementptr inbounds i8, ptr addrspace(1) %92, i16 6
  %94 = load i16, ptr addrspace(1) %93, !tbaa !9
  %95 = icmp ne i16 %94, %4
  br i1 %95, label %b32, label %b30

b30:
  %96 = getelementptr inbounds i8, ptr addrspace(1) %92, i16 2
  %97 = load i16, ptr addrspace(1) %96, !tbaa !9
  switch i16 %97, label %b32 [
    i16 0, label %b34
    i16 1, label %b34
    i16 2, label %b35
    i16 7, label %b36
    i16 8, label %b37
    i16 9, label %b38
    i16 10, label %b39
  ]

b32:
  %98 = add nsw i16 %85, 1
  store i16 %98, ptr %10, !tbaa !9
  br label %b27

b34:
  %99 = load ptr addrspace(1), ptr %89, !tbaa !23
  %100 = getelementptr inbounds i8, ptr addrspace(1) %99, i16 %91
  %101 = getelementptr inbounds i8, ptr addrspace(1) %100, i16 10
  %102 = load i16, ptr addrspace(1) %101, !tbaa !9
  %103 = icmp eq i16 %102, 0
  br i1 %103, label %b45, label %b32

b35:
  %104 = getelementptr inbounds i8, ptr addrspace(1) %92, i16 10
  %105 = load i16, ptr addrspace(1) %104, !tbaa !9
  %106 = icmp ne i16 %105, 4
  br i1 %106, label %b41, label %b32

b36:
  %107 = load ptr addrspace(1), ptr %89, !tbaa !23
  %108 = getelementptr inbounds i8, ptr addrspace(1) %107, i16 %91
  %109 = getelementptr inbounds i8, ptr addrspace(1) %108, i16 10
  %110 = load i16, ptr addrspace(1) %109, !tbaa !9
  %111 = icmp eq i16 %110, 0
  br i1 %111, label %b49, label %b32

b37:
  call cc 1005 addrspace(1) void @"_ent_trig_fire@3"(ptr %0, ptr %1, ptr %2, ptr %3, i16 %85)
  br label %b32

b38:
  %112 = getelementptr inbounds i8, ptr addrspace(1) %92, i16 10
  %113 = load i16, ptr addrspace(1) %112, !tbaa !9
  %114 = icmp eq i16 %113, 0
  br i1 %114, label %b47, label %b36

b39:
  call cc 1005 void @"_ent_lightning@3"(ptr %0, ptr %1, ptr %3, i16 %85)
  br label %b32

b41:
  %115 = getelementptr inbounds i8, ptr addrspace(1) %92, i16 12
  %116 = load i16, ptr addrspace(1) %115, !tbaa !9
  %117 = sub nsw i16 %116, 1
  store i16 %117, ptr addrspace(1) %115, !tbaa !9
  %118 = load ptr addrspace(1), ptr %89, !tbaa !23
  %119 = getelementptr inbounds i8, ptr addrspace(1) %118, i16 %91
  %120 = getelementptr inbounds i8, ptr addrspace(1) %119, i16 12
  %121 = load i16, ptr addrspace(1) %120, !tbaa !9
  %122 = icmp sle i16 %121, 0
  br i1 %122, label %b43, label %b32

b43:
  call cc 1005 addrspace(1) void @"_ent_trig_fire@3"(ptr %0, ptr %1, ptr %2, ptr %3, i16 %85)
  br label %b32

b45:
  call cc 1005 addrspace(1) void @"_ent_trig_fire@3"(ptr %0, ptr %1, ptr %2, ptr %3, i16 %85)
  br label %b32

b47:
  %123 = getelementptr inbounds i8, ptr addrspace(1) %92, i16 42
  %124 = load i32, ptr addrspace(1) %123, !tbaa !7
  store i32 %124, ptr %12, !tbaa !7
  %125 = getelementptr inbounds i8, ptr addrspace(1) %123, i16 4
  %126 = getelementptr inbounds i8, ptr %12, i16 4
  %127 = load i32, ptr addrspace(1) %125, !tbaa !7
  store i32 %127, ptr %126, !tbaa !7
  %128 = getelementptr inbounds i8, ptr addrspace(1) %123, i16 8
  %129 = getelementptr inbounds i8, ptr %12, i16 8
  %130 = load i32, ptr addrspace(1) %128, !tbaa !7
  store i32 %130, ptr %129, !tbaa !7
  %131 = add nsw i16 2048, %85
  %132 = getelementptr inbounds i8, ptr %0, i16 7026
  %133 = call cc 1005 addrspace(1) i16 @"_snd_start@3"(ptr %132, ptr %1, i16 %131, i16 1, i16 163, ptr %12, i16 1)
  %134 = call cc 1005 addrspace(1) i16 @"_snd_start@3"(ptr %132, ptr %1, i16 %131, i16 2, i16 164, ptr %12, i16 1)
  %135 = call cc 1005 addrspace(1) i16 @"_part_lava@3"(ptr %3, ptr %12)
  %136 = load ptr addrspace(1), ptr %89, !tbaa !23
  %137 = getelementptr inbounds i8, ptr addrspace(1) %136, i16 %91
  %138 = getelementptr inbounds i8, ptr addrspace(1) %137, i16 40
  store i16 0, ptr addrspace(1) %138, !tbaa !9
  %139 = getelementptr inbounds i8, ptr %3, i16 14
  %140 = load float, ptr %139, !tbaa !15
  %141 = getelementptr inbounds i8, ptr addrspace(1) %137, i16 20
  store float %140, ptr addrspace(1) %141, !tbaa !15
  %142 = load float, ptr %139, !tbaa !15
  %143 = fadd float %142, 0x3FFB333340000000
  %144 = fadd float %143, 0x3FE99999A0000000
  %145 = getelementptr inbounds i8, ptr addrspace(1) %137, i16 68
  store float %144, ptr addrspace(1) %145, !tbaa !15
  %146 = getelementptr inbounds i8, ptr addrspace(1) %137, i16 28
  store float 1.000000e+00, ptr addrspace(1) %146, !tbaa !15
  %147 = getelementptr inbounds i8, ptr addrspace(1) %137, i16 36
  store float 0.000000e+00, ptr addrspace(1) %147, !tbaa !15
  br label %b36

b49:
  store i16 5, ptr addrspace(1) %109, !tbaa !9
  br label %b32
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!7 = !{!6, !6, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
!10 = !{!"int4", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!"int8", !6, i64 0}
!13 = !{!12, !12, i64 0}
!14 = !{!"float4", !6, i64 0}
!15 = !{!14, !14, i64 0}
!16 = !{!"float8", !6, i64 0}
!17 = !{!16, !16, i64 0}
!18 = !{!"float10", !6, i64 0}
!19 = !{!18, !18, i64 0}
!20 = !{!"pointer2", !6, i64 0}
!21 = !{!20, !20, i64 0}
!22 = !{!"pointer4", !6, i64 0}
!23 = !{!22, !22, i64 0}
