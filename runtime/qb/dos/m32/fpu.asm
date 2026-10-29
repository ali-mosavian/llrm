;; name: B$ATAN2, B$SINCOS
;; desc: the FPU's arctangent and sine
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (the target's own convention: eax, edx, ebx,
;;       ecx).

                .386
                .387
                .model  flat

                public  B$ATAN2
                public  B$SINCOS

.code
;; B$ATAN2 - store atan(y / x), in (-pi, pi], at `out`.
;;
;; The FPU stack is left empty.
;;
;; Args:
;;     y   (eax): ptr to a double.
;;     x   (edx): ptr to a double.
;;     out (ebx): ptr to a double.
;;
;; Returns:
;;     *out: the angle.
;;
;; Saves:    none
;; Clobbers: flags, the FPU's status
B$ATAN2         proc

                fld     qword ptr [eax]
                fld     qword ptr [edx]
                fpatan                          ;; atan(st1 / st0), in (-pi, pi]
                fstp    qword ptr [ebx]
                fwait
                ret
B$ATAN2         endp

;; B$SINCOS - store the sine and the cosine of an angle.
;;
;; The FPU stack is left empty.
;;
;; Args:
;;     angle  (eax): ptr to a double.
;;     sine   (edx): ptr to a double.
;;     cosine (ebx): ptr to a double.
;;
;; Returns:
;;     *sine, *cosine.
;;
;; Saves:    none
;; Clobbers: flags, the FPU's status
B$SINCOS        proc

                fld     qword ptr [eax]
                fsincos                         ;; st0 the cosine, st1 the sine
                fstp    qword ptr [ebx]
                fstp    qword ptr [edx]
                fwait
                ret
B$SINCOS        endp
                end
