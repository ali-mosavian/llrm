define i32 @f(i32 %n, i1 %flag) {
entry:
  br label %loop

next.si.unfold.false:
  %.si.unfold.phi = phi i32 [ 2, %latch ]
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ 1, %latch ], [ %.si.unfold.phi, %next.si.unfold.false ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ], [ %i.next, %next.si.unfold.false ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %body, label %done

body:
  %v0 = add i32 %i, %n
  %v1 = add i32 %v0, 1
  %v2 = add i32 %v1, 2
  %v3 = add i32 %v2, 3
  %v4 = add i32 %v3, 4
  %v5 = add i32 %v4, 5
  %v6 = add i32 %v5, 6
  %v7 = add i32 %v6, 7
  %v8 = add i32 %v7, 8
  %v9 = add i32 %v8, 9
  %v10 = add i32 %v9, 10
  %v11 = add i32 %v10, 11
  %v12 = add i32 %v11, 12
  %v13 = add i32 %v12, 13
  %v14 = add i32 %v13, 14
  %v15 = add i32 %v14, 15
  %v16 = add i32 %v15, 16
  %v17 = add i32 %v16, 17
  %v18 = add i32 %v17, 18
  %v19 = add i32 %v18, 19
  %v20 = add i32 %v19, 20
  %v21 = add i32 %v20, 21
  %v22 = add i32 %v21, 22
  %v23 = add i32 %v22, 23
  %v24 = add i32 %v23, 24
  %v25 = add i32 %v24, 25
  %v26 = add i32 %v25, 26
  %v27 = add i32 %v26, 27
  %v28 = add i32 %v27, 28
  %v29 = add i32 %v28, 29
  %v30 = add i32 %v29, 30
  %v31 = add i32 %v30, 31
  %v32 = add i32 %v31, 32
  %v33 = add i32 %v32, 33
  %v34 = add i32 %v33, 34
  %v35 = add i32 %v34, 35
  %v36 = add i32 %v35, 36
  %v37 = add i32 %v36, 37
  %v38 = add i32 %v37, 38
  %v39 = add i32 %v38, 39
  %v40 = add i32 %v39, 40
  %v41 = add i32 %v40, 41
  %v42 = add i32 %v41, 42
  %v43 = add i32 %v42, 43
  %v44 = add i32 %v43, 44
  %v45 = add i32 %v44, 45
  %v46 = add i32 %v45, 46
  %v47 = add i32 %v46, 47
  %v48 = add i32 %v47, 48
  %v49 = add i32 %v48, 49
  %v50 = add i32 %v49, 50
  %v51 = add i32 %v50, 51
  %v52 = add i32 %v51, 52
  %v53 = add i32 %v52, 53
  %v54 = add i32 %v53, 54
  %v55 = add i32 %v54, 55
  %v56 = add i32 %v55, 56
  %v57 = add i32 %v56, 57
  %v58 = add i32 %v57, 58
  %v59 = add i32 %v58, 59
  %v60 = add i32 %v59, 60
  %v61 = add i32 %v60, 61
  %v62 = add i32 %v61, 62
  %v63 = add i32 %v62, 63
  %v64 = add i32 %v63, 64
  %v65 = add i32 %v64, 65
  %v66 = add i32 %v65, 66
  %v67 = add i32 %v66, 67
  %v68 = add i32 %v67, 68
  %v69 = add i32 %v68, 69
  %v70 = add i32 %v69, 70
  %v71 = add i32 %v70, 71
  %v72 = add i32 %v71, 72
  %v73 = add i32 %v72, 73
  %v74 = add i32 %v73, 74
  %v75 = add i32 %v74, 75
  %v76 = add i32 %v75, 76
  %v77 = add i32 %v76, 77
  %v78 = add i32 %v77, 78
  %v79 = add i32 %v78, 79
  %v80 = add i32 %v79, 80
  %v81 = add i32 %v80, 81
  %v82 = add i32 %v81, 82
  %v83 = add i32 %v82, 83
  %v84 = add i32 %v83, 84
  %v85 = add i32 %v84, 85
  %v86 = add i32 %v85, 86
  %v87 = add i32 %v86, 87
  %v88 = add i32 %v87, 88
  %v89 = add i32 %v88, 89
  %v90 = add i32 %v89, 90
  %v91 = add i32 %v90, 91
  %v92 = add i32 %v91, 92
  %v93 = add i32 %v92, 93
  %v94 = add i32 %v93, 94
  %v95 = add i32 %v94, 95
  %v96 = add i32 %v95, 96
  %v97 = add i32 %v96, 97
  %v98 = add i32 %v97, 98
  %v99 = add i32 %v98, 99
  %v100 = add i32 %v99, 100
  %v101 = add i32 %v100, 101
  %v102 = add i32 %v101, 102
  %v103 = add i32 %v102, 103
  %v104 = add i32 %v103, 104
  %v105 = add i32 %v104, 105
  %v106 = add i32 %v105, 106
  %v107 = add i32 %v106, 107
  %v108 = add i32 %v107, 108
  %v109 = add i32 %v108, 109
  %v110 = add i32 %v109, 110
  %v111 = add i32 %v110, 111
  %v112 = add i32 %v111, 112
  %v113 = add i32 %v112, 113
  %v114 = add i32 %v113, 114
  %v115 = add i32 %v114, 115
  %v116 = add i32 %v115, 116
  %v117 = add i32 %v116, 117
  %v118 = add i32 %v117, 118
  %v119 = add i32 %v118, 119
  %v120 = add i32 %v119, 120
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
    i32 2, label %case2
  ]

case0:
  br label %latch

case1:
  br label %latch

case2:
  br label %latch

case.default:
  br label %latch

latch:
  %i.next = add i32 %i, 1
  br i1 %flag, label %loop, label %next.si.unfold.false

done:
  ret i32 %i
}
