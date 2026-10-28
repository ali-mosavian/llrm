;; name: B$ATAN2, B$SINCOS, B$CHKEXP, B$CHKEX2, B$CHKLOG
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
                public  B$CHKEXP
                public  B$CHKEX2
                public  B$CHKLOG

                extrn           B$SERR:near

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
;; B$CHKEXP - ERROR 6 where the operand is 22712.7002... or more.
;;
;; The operand is ST0 and stays there; no stack bytes are used (the
;; compiler's range check of EXP, which was a compare and a raising block at every use).
;; A NaN passes, as it did inline. On the error the FPU stack is popped.
;;
;; Saves:    all but ax
;; Clobbers: ax, flags, the FPU's status
B$CHKEXP        proc    public

                fcom    qword ptr [expmax]
                fnstsw  ax
                sahf
                jae     B$CHKEXP_raise
                ret
B$CHKEXP_raise:
                fstp    st(0)
                push    6
                call    B$SERR
B$CHKEXP        endp

;; B$CHKEX2 - ERROR 6 where the operand is 32767.5 or more.
;;
;; The operand is ST0 and stays there; no stack bytes are used (the
;; compiler's range check of EXP and ^, which was a compare and a raising block at every use).
;; A NaN passes, as it did inline. On the error the FPU stack is popped.
;;
;; Saves:    all but ax
;; Clobbers: ax, flags, the FPU's status
B$CHKEX2        proc    public

                fcom    qword ptr [exp2max]
                fnstsw  ax
                sahf
                jae     B$CHKEX2_raise
                ret
B$CHKEX2_raise:
                fstp    st(0)
                push    6
                call    B$SERR
B$CHKEX2        endp

;; B$CHKLOG - ERROR 5 where the operand is zero or negative (LOG).
;;
;; The operand is ST0 and stays there. One FPU slot is used above it. A NaN
;; passes, as it did inline. On the error the FPU stack is popped.
;;
;; Saves:    all but ax
;; Clobbers: ax, flags, the FPU's status
B$CHKLOG        proc    public

                fldz
                fcomp
                fnstsw  ax
                sahf
                jae     B$CHKLOG_raise
                ret
B$CHKLOG_raise:
                fstp    st(0)
                push    5
                call    B$SERR
B$CHKLOG        endp

expmax          dq      22712.700238998008
exp2max         dq      32767.5

                end
