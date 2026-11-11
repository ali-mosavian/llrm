' flags: -Os | -O2
' A SUB's locals are zero when the runtime builds the frame (B$ENRA fills it),
' wherever the frame layout puts them among the spill slots: forty longs, kept
' in memory by a by-reference call, are read before they are written and then
' summed in a loop that keeps more values live than there are registers.
DEFINT A-Z
DECLARE SUB Probe (n)
DECLARE SUB Keep (x&)
Probe 3
END

SUB Keep (x&)
END SUB

SUB Probe (n)
    FOR i = 1 TO n
        w1& = w1& + i * 2 + v1&
        w2& = w2& + i * 3 + v2&
        w3& = w3& + i * 4 + v3&
    NEXT
    PRINT "S"; v1&
    t& = w1& + w2& + w3&
    PRINT "T"; t&
END SUB


