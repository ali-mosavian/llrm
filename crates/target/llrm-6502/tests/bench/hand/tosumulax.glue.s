; Two-line glue: tosumulax takes the first operand on the C stack and the second in A/X; C-callable alias.
        .export _hand_tosumulax
        .import tosumulax
_hand_tosumulax:
        jmp     tosumulax
