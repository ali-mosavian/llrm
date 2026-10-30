;; name: B$INT10, B$INT10R, B$INT16, B$INT17, B$INT21
;; desc: software interrupts
;;
;; chng: oct/26 written [ali]
;; obs.: DOS/32A passes the interrupt on to the real-mode BIOS and DOS with
;;       these registers; the calls the runtime makes this way take no pointers.

                .386
                .model  flat

                public  B$INT10
                public  B$INT16
                public  B$INT17
                public  B$INT21
                public  B$INT10R

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
;; CALLINT - software interrupt, with the registers of the Regs at esi.
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
                ;; an interrupt returns 16-bit registers: the rest is not its
                movzx   eax, ax
                mov     [esi].Regs.rax, eax
                movzx   ebx, bx
                mov     [esi].Regs.rbx, ebx
                movzx   ecx, cx
                mov     [esi].Regs.rcx, ecx
                movzx   edx, dx
                mov     [esi].Regs.rdx, edx
                movzx   ebp, bp
                mov     [esi].Regs.rbp, ebp
                mov     eax, es
                mov     [esi].Regs.res, eax
                pushfd
                pop     [esi].Regs.rflags
                pop     es
                popad
                ret
                endm

;; B$INT10 - call BIOS or DOS interrupt 10h.
;;
;; Loads eax ebx ecx edx from `regs`, runs interrupt 10h, and stores ax bx cx dx
;; ebp es and the flags back as the interrupt left them.
;;
;; Args:
;;     regs (eax): ptr to a Regs.
;;
;; Returns:
;;     regs: the registers and the flags as the interrupt left them.
;;
;; Saves:    all general registers and es (pushad, push es)
;; Clobbers: flags
B$INT10         proc

                CALLINT 10h
B$INT10         endp

;; B$INT10R - call BIOS interrupt 10h in real mode (DPMI 0300h).
;;
;; For the functions that return a real-mode segment:offset (the font's, 1130h),
;; which a protected-mode call would not give as the BIOS made it.
;;
;; Args:
;;     regs (eax): ptr to a Regs.
;;
;; Returns:
;;     regs: the registers and the flags as the interrupt left them; res and rbp
;;         are the real-mode ES and BP.
;;
;; Saves:    all general registers and es (pushad, push es)
;; Clobbers: flags
RMCALL_SIZE     equ     32h

B$INT10R        proc

                pushad
                push    es
                mov     esi, eax
                sub     esp, RMCALL_SIZE
                mov     edi, esp
                xor     eax, eax
                mov     ecx, RMCALL_SIZE
                rep     stosb
                mov     edi, esp
                mov     eax, [esi].Regs.rax
                mov     [edi+1Ch], eax
                mov     eax, [esi].Regs.rbx
                mov     [edi+10h], eax
                mov     eax, [esi].Regs.rcx
                mov     [edi+18h], eax
                mov     eax, [esi].Regs.rdx
                mov     [edi+14h], eax
                mov     eax, [esi].Regs.rbp
                mov     [edi+08h], eax
                mov     eax, 0300h
                ;; interrupt 10h, no flags to reset
                mov     ebx, 0010h
                xor     ecx, ecx
                int     31h
                movzx   eax, word ptr [edi+1Ch]
                mov     [esi].Regs.rax, eax
                movzx   eax, word ptr [edi+10h]
                mov     [esi].Regs.rbx, eax
                movzx   eax, word ptr [edi+18h]
                mov     [esi].Regs.rcx, eax
                movzx   eax, word ptr [edi+14h]
                mov     [esi].Regs.rdx, eax
                movzx   eax, word ptr [edi+08h]
                mov     [esi].Regs.rbp, eax
                movzx   eax, word ptr [edi+22h]
                mov     [esi].Regs.res, eax
                movzx   eax, word ptr [edi+20h]
                mov     [esi].Regs.rflags, eax
                add     esp, RMCALL_SIZE
                pop     es
                popad
                ret
B$INT10R        endp

;; B$INT17 - call BIOS or DOS interrupt 17h.
;;
;; Loads eax ebx ecx edx from `regs`, runs interrupt 17h, and stores ax bx cx dx
;; ebp es and the flags back as the interrupt left them.
;;
;; Args:
;;     regs (eax): ptr to a Regs.
;;
;; Returns:
;;     regs: the registers and the flags as the interrupt left them.
;;
;; Saves:    all general registers and es (pushad, push es)
;; Clobbers: flags
B$INT17         proc

                CALLINT 17h
B$INT17         endp

;; B$INT16 - call BIOS or DOS interrupt 16h.
;;
;; Loads eax ebx ecx edx from `regs`, runs interrupt 16h, and stores ax bx cx dx
;; ebp es and the flags back as the interrupt left them.
;;
;; Args:
;;     regs (eax): ptr to a Regs.
;;
;; Returns:
;;     regs: the registers and the flags as the interrupt left them.
;;
;; Saves:    all general registers and es (pushad, push es)
;; Clobbers: flags
B$INT16         proc

                CALLINT 16h
B$INT16         endp

;; B$INT21 - call BIOS or DOS interrupt 21h.
;;
;; Loads eax ebx ecx edx from `regs`, runs interrupt 21h, and stores ax bx cx dx
;; ebp es and the flags back as the interrupt left them.
;;
;; Args:
;;     regs (eax): ptr to a Regs.
;;
;; Returns:
;;     regs: the registers and the flags as the interrupt left them.
;;
;; Saves:    all general registers and es (pushad, push es)
;; Clobbers: flags
B$INT21         proc

                CALLINT 21h
B$INT21         endp
                end
