; Two-line glue: tosudivax takes the first operand on the C stack and the second in A/X; C-callable alias.
        .export _hand_tosudivax
        .import tosudivax
_hand_tosudivax:
        jmp     tosudivax
