;; name: B$ATAN2, B$SINCOS, B$CHKEXP, B$CHKEX2, B$CHKLOG
;; desc: the FPU's arctangent and sine
;;
;; chng: oct/26 written [ali]
;; obs.: the registers are C's (regparm3: ax, dx, cx), so no thunk is needed;
;;       the QB runtime's device code (dev*.c, gfxdev.c) is C around these.
;;       Target 386+ with an FPU.

                .model  medium, pascal
                .386
                .387
                option  proc:private

                include qb.inc

                extrn           B$SERR:far

.code
;; B$ATAN2 - store atan(y / x), in (-pi, pi], at `out`.
;;
;; The FPU stack is left empty.
;;
;; Args:
;;     y   (ax): ptr to a double.
;;     x   (dx): ptr to a double.
;;     out (cx): ptr to a double.
;;
;; Returns:
;;     *out: the angle.
;;
;; Saves:    si
;; Clobbers: bx, flags, the FPU's status
B$ATAN2         proc    public uses si

                mov     si, cx
                mov     bx, ax
                fld     qword ptr [bx]
                mov     bx, dx
                fld     qword ptr [bx]
                fpatan                          ;; atan(st1 / st0), in (-pi, pi]
                fstp    qword ptr [si]
                fwait
                ret
B$ATAN2         endp

;; B$SINCOS - store the sine and the cosine of an angle.
;;
;; The FPU stack is left empty.
;;
;; Args:
;;     angle  (ax): ptr to a double.
;;     sine   (dx): ptr to a double.
;;     cosine (cx): ptr to a double.
;;
;; Returns:
;;     *sine, *cosine.
;;
;; Saves:    si
;; Clobbers: bx, flags, the FPU's status
B$SINCOS        proc    public uses si

                mov     si, cx
                mov     bx, ax
                fld     qword ptr [bx]
                fsincos                         ;; st0 the cosine, st1 the sine
                fstp    qword ptr [si]
                mov     bx, dx
                fstp    qword ptr [bx]
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

                fcom    qword ptr cs:[expmax]
                fnstsw  ax
                sahf
                jae     B$CHKEXP_raise
                ret
B$CHKEXP_raise:
                fstp    st(0)
                push    6
                call    far ptr B$SERR
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

                fcom    qword ptr cs:[exp2max]
                fnstsw  ax
                sahf
                jae     B$CHKEX2_raise
                ret
B$CHKEX2_raise:
                fstp    st(0)
                push    6
                call    far ptr B$SERR
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
                call    far ptr B$SERR
B$CHKLOG        endp

expmax          dq      22712.700238998008
exp2max         dq      32767.5

                end
