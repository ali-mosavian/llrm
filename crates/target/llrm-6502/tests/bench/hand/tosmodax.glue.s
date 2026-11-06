; Two-line glue: tosmodax takes the first operand on the C stack and the second in A/X; C-callable alias.
        .export _hand_tosmodax
        .import tosmodax
_hand_tosmodax:
        jmp     tosmodax
