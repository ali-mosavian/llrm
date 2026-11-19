; Two-line glue: tosmulax takes the first operand on the C stack and the second in A/X; C-callable alias.
        .export _hand_tosmulax
        .import tosmulax
_hand_tosmulax:
        jmp     tosmulax
