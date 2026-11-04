; Two-line glue: tosdivax takes the first operand on the C stack and the second in A/X; C-callable alias.
        .export _hand_tosdivax
        .import tosdivax
_hand_tosdivax:
        jmp     tosdivax
