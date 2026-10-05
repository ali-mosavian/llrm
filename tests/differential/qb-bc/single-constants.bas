' bc: /O /E /X
' BC folds a constant subexpression at compile time to its type (a SINGLE one is a SINGLE
' constant, as CONST's is); run-time arithmetic keeps the x87's precision. Prints each result's DOUBLE bytes.
DEFINT I-N
w = -1: v = 7
CONST k = .5 / 7
PRINT HEX$(CVL(LEFT$(MKD$(k * 1#), 4))); " "; HEX$(CVL(RIGHT$(MKD$(k * 1#), 4)))
d# = (1 / 3) * 1#
PRINT HEX$(CVL(LEFT$(MKD$(d#), 4))); " "; HEX$(CVL(RIGHT$(MKD$(d#), 4)))
d# = (.5 / 7) * w * 1#
PRINT HEX$(CVL(LEFT$(MKD$(d#), 4))); " "; HEX$(CVL(RIGHT$(MKD$(d#), 4)))
d# = .5 * (w / v) * 1#
PRINT HEX$(CVL(LEFT$(MKD$(d#), 4))); " "; HEX$(CVL(RIGHT$(MKD$(d#), 4)))
d# = (1 / 3) + 1#
PRINT HEX$(CVL(LEFT$(MKD$(d#), 4))); " "; HEX$(CVL(RIGHT$(MKD$(d#), 4)))
s! = 1 / 3
d# = s! * 1#
PRINT HEX$(CVL(LEFT$(MKD$(d#), 4))); " "; HEX$(CVL(RIGHT$(MKD$(d#), 4)))
d# = 1# / 3
PRINT HEX$(CVL(LEFT$(MKD$(d#), 4))); " "; HEX$(CVL(RIGHT$(MKD$(d#), 4)))
d# = (1 / 3#) * 1#
PRINT HEX$(CVL(LEFT$(MKD$(d#), 4))); " "; HEX$(CVL(RIGHT$(MKD$(d#), 4)))
d# = (2 * 0.1) * 1#
PRINT HEX$(CVL(LEFT$(MKD$(d#), 4))); " "; HEX$(CVL(RIGHT$(MKD$(d#), 4)))
