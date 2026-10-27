;; B$CHKEXP, B$CHKEX2, B$CHKLOG at their range edges, hand-called (medium model, 16-bit).
;; Prints one digit per case: the error raised (0, 5 or 6), then "ok" or "FAIL"; exit 0 on ok.
;; B$SERR here records its argument and resumes after the call; the helper must have popped the FPU stack.

                .model  medium, pascal
                .386
                .387
                option  casemap:none

                extrn   B$CHKEXP:far
                extrn   B$CHKEX2:far
                extrn   B$CHKLOG:far

.stack   400h
.data
saved_sp        dw      0
resume          dw      0
raised          dw      0
fails           dw      0
expected        db      '0','0','0','6','6','6','0','0','6','0','0','0','0','5','5','5','0'
cases           = $ - expected
shown           db      cases dup (?), '$'
okmsg           db      ' ok', 13, 10, '$'
failmsg         db      ' FAIL', 13, 10, '$'
nextup          dd      0d0b73af4h, 040d62e2ch
exp_bound       dd      0d0b73af5h, 040d62e2ch
past_bound      dd      0d0b73af6h, 040d62e2ch
two             dq      32767.49
two_bound       dq      32767.5
zero            dq      0.0
negzero         dq      -0.0
minus           dq      -1.0
tiny            dd      000000001h, 000000000h
nan             dd      0, 0fff80000h
inf             dd      0, 07ff00000h
big_neg         dq      -1.0e308

;; case table: function, operand
table           dd      B$CHKEXP, zero
                dd      B$CHKEXP, big_neg
                dd      B$CHKEXP, nextup
                dd      B$CHKEXP, exp_bound
                dd      B$CHKEXP, past_bound
                dd      B$CHKEXP, inf
                dd      B$CHKEXP, nan
                dd      B$CHKEX2, two
                dd      B$CHKEX2, two_bound
                dd      B$CHKEX2, nan
                dd      B$CHKLOG, tiny
                dd      B$CHKLOG, inf
                dd      B$CHKLOG, nan
                dd      B$CHKLOG, zero
                dd      B$CHKLOG, negzero
                dd      B$CHKLOG, minus
                dd      B$CHKLOG, nan

.code
B$SERR          proc    far pascal public errnum:word
                mov     ax, errnum
                mov     bx, @data
                mov     ds, bx
                mov     raised, ax
                mov     sp, saved_sp
                jmp     word ptr resume
B$SERR          endp

start:
                mov     ax, @data
                mov     ds, ax
                finit
                xor     si, si                  ;; case index
next:
                mov     raised, 0
                mov     saved_sp, sp
                mov     resume, offset back
                mov     bx, si
                shl     bx, 3
                les     di, dword ptr table[bx + 4]
                fld     qword ptr es:[di]
                call    dword ptr table[bx]
                fstp    st(0)
back:
                fnstsw  ax                      ;; the FPU stack must be empty again
                test    ax, 3800h
                jz      empty
                inc     fails
                finit
empty:
                mov     ax, raised
                add     al, '0'
                mov     shown[si], al
                cmp     al, expected[si]
                je      same
                inc     fails
same:
                inc     si
                cmp     si, cases
                jb      next
                mov     dx, offset shown
                mov     ah, 9
                int     21h
                mov     dx, offset okmsg
                cmp     fails, 0
                je      say
                mov     dx, offset failmsg
say:
                mov     ah, 9
                int     21h
                mov     ax, 4c00h
                cmp     fails, 0
                je      quit
                mov     al, 1
quit:
                int     21h
                end     start
