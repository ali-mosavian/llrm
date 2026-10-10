;; name: B$INT10, B$INT17, B$INT21
;; desc: software interrupts
;;
;; args: eax:ptr Regs | the interrupt, with the registers from Regs
;; retn: Regs -> the registers and the flags as the interrupt left them
;;
;; chng: oct/26 written [ali]
;; obs.: DOS/32A passes the interrupt on to the real-mode BIOS and DOS with these registers; the calls the
;;       runtime makes this way take no pointers.  All registers are kept but the ones Regs returns.

                .386
                .model  flat

                public  B$INT10
                public  B$INT17
                public  B$INT21

Regs            struc
                rax             dword   ?
                rbx             dword   ?
                rcx             dword   ?
                rdx             dword   ?
                rbp             dword   ?
                res             dword   ?
                rflags          dword   ?
Regs            ends

.code
;;::::::::::::::
;; software interrupt, with the registers of the Regs at esi
CALLINT         macro   vector:req

                pushad
                push    es
                mov     esi, eax
                push    esi
                mov     eax, [esi].Regs.rax
                mov     ebx, [esi].Regs.rbx
                mov     ecx, [esi].Regs.rcx
                mov     edx, [esi].Regs.rdx
                int     vector
                pop     esi
                mov     [esi].Regs.rax, eax
                mov     [esi].Regs.rbx, ebx
                mov     [esi].Regs.rcx, ecx
                mov     [esi].Regs.rdx, edx
                mov     [esi].Regs.rbp, ebp
                mov     eax, es
                mov     [esi].Regs.res, eax
                pushfd
                pop     [esi].Regs.rflags
                pop     es
                popad
                ret
                endm

;;::::::::::::::
;; B$INT10 (eax: regs)
B$INT10         proc

                CALLINT 10h
B$INT10         endp

;;::::::::::::::
;; B$INT17 (eax: regs)
B$INT17         proc

                CALLINT 17h
B$INT17         endp

;;::::::::::::::
;; B$INT21 (eax: regs)
B$INT21         proc

                CALLINT 21h
B$INT21         endp
                end
