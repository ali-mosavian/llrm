; Two-line glue: tosumodax takes the first operand on the C stack and the second in A/X; C-callable alias.
        .export _hand_tosumodax
        .import tosumodax
_hand_tosumodax:
        jmp     tosumodax
