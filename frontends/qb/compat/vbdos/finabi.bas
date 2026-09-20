option explicit

declare function DDB# (byval cost#, byval salvage#, byval life#, byval period#, statusValue as integer)
declare function FV# (byval rateValue#, byval periods#, byval payment#, byval presentValue#, byval typeValue as integer, statusValue as integer)
declare function IPmt# (byval rateValue#, byval period#, byval periods#, byval presentValue#, byval futureValue#, byval typeValue as integer, statusValue as integer)
declare function IRR# (values#(), byval countValue as integer, byval guessValue#, statusValue as integer)
declare function MIRR# (values#(), byval countValue as integer, byval financeRate#, byval reinvestRate#, statusValue as integer)
declare function NPer# (byval rateValue#, byval payment#, byval presentValue#, byval futureValue#, byval typeValue as integer, statusValue as integer)
declare function NPV# (byval rateValue#, values#(), byval countValue as integer, statusValue as integer)
declare function Pmt# (byval rateValue#, byval periods#, byval presentValue#, byval futureValue#, byval typeValue as integer, statusValue as integer)
declare function PPmt# (byval rateValue#, byval period#, byval periods#, byval presentValue#, byval futureValue#, byval typeValue as integer, statusValue as integer)
declare function PV# (byval rateValue#, byval periods#, byval payment#, byval futureValue#, byval typeValue as integer, statusValue as integer)
declare function Rate# (byval periods#, byval payment#, byval presentValue#, byval futureValue#, byval typeValue as integer, byval guessValue#, statusValue as integer)
declare function SLN# (byval cost#, byval salvage#, byval life#, statusValue as integer)
declare function SYD# (byval cost#, byval salvage#, byval life#, byval period#, statusValue as integer)

dim values(1 to 3) as double
dim statusValues(1 to 13) as integer
dim resultValues(1 to 13) as double
dim resultIndex as integer

values(1) = -100
values(2) = 60
values(3) = 60
resultValues(1) = DDB#(100, 10, 9, 1, statusValues(1))
resultValues(2) = FV#(.01, 12, -10, 0, 0, statusValues(2))
resultValues(3) = IPmt#(.01, 1, 12, 100, 0, 0, statusValues(3))
resultValues(4) = IRR#(values(), 3, .1, statusValues(4))
resultValues(5) = MIRR#(values(), 3, .1, .1, statusValues(5))
resultValues(6) = NPer#(.01, -10, 100, 0, 0, statusValues(6))
resultValues(7) = NPV#(.01, values(), 3, statusValues(7))
resultValues(8) = Pmt#(.01, 12, 100, 0, 0, statusValues(8))
resultValues(9) = PPmt#(.01, 1, 12, 100, 0, 0, statusValues(9))
resultValues(10) = PV#(.01, 12, -10, 0, 0, statusValues(10))
resultValues(11) = Rate#(12, -10, 100, 0, 0, .01, statusValues(11))
resultValues(12) = SLN#(100, 10, 9, statusValues(12))
resultValues(13) = SYD#(100, 10, 9, 1, statusValues(13))
for resultIndex = 1 to 13
    if statusValues(resultIndex) <> 0 then
        print "FAIL financial status"; resultIndex; statusValues(resultIndex)
        end
    end if
next resultIndex
if mkd$(resultValues(1)) <> chr$(142) + chr$(227) + chr$(56) + chr$(142) + chr$(227) + chr$(56) + chr$(54) + chr$(64) then
    print "FAIL financial ddb"
    end
end if
if mkd$(resultValues(2)) <> chr$(146) + chr$(110) + chr$(133) + chr$(74) + chr$(205) + chr$(180) + chr$(95) + chr$(64) then
    print "FAIL financial fv"
    end
end if
if mkd$(resultValues(3)) <> chr$(0) + chr$(0) + chr$(0) + chr$(244) + chr$(255) + chr$(255) + chr$(239) + chr$(191) then
    print "FAIL financial ipmt"
    end
end if
if mkd$(resultValues(4)) <> chr$(154) + chr$(102) + chr$(248) + chr$(137) + chr$(139) + chr$(185) + chr$(192) + chr$(63) then
    print "FAIL financial irr"
    end
end if
if mkd$(resultValues(5)) <> chr$(64) + chr$(116) + chr$(107) + chr$(66) + chr$(250) + chr$(91) + chr$(191) + chr$(63) then
    print "FAIL financial mirr"
    end
end if
if mkd$(resultValues(6)) <> chr$(47) + chr$(88) + chr$(3) + chr$(206) + chr$(98) + chr$(45) + chr$(37) + chr$(64) then
    print "FAIL financial nper"
    end
end if
if mkd$(resultValues(7)) <> chr$(252) + chr$(69) + chr$(29) + chr$(205) + chr$(19) + chr$(11) + chr$(50) + chr$(64) then
    print "FAIL financial npv"
    end
end if
if mkd$(resultValues(8)) <> chr$(98) + chr$(193) + chr$(96) + chr$(215) + chr$(14) + chr$(197) + chr$(33) + chr$(192) then
    print "FAIL financial pmt"
    end
end if
if mkd$(resultValues(9)) <> chr$(196) + chr$(130) + chr$(65) + chr$(176) + chr$(29) + chr$(138) + chr$(31) + chr$(192) then
    print "FAIL financial ppmt"
    end
end if
if mkd$(resultValues(10)) <> chr$(196) + chr$(19) + chr$(87) + chr$(229) + chr$(63) + chr$(35) + chr$(92) + chr$(64) then
    print "FAIL financial pv"
    end
end if
if mkd$(resultValues(11)) <> chr$(196) + chr$(71) + chr$(207) + chr$(42) + chr$(22) + chr$(238) + chr$(157) + chr$(63) then
    print "FAIL financial rate"
    end
end if
if mkd$(resultValues(12)) <> chr$(0) + chr$(0) + chr$(0) + chr$(0) + chr$(0) + chr$(0) + chr$(36) + chr$(64) then
    print "FAIL financial sln"
    end
end if
if mkd$(resultValues(13)) <> chr$(0) + chr$(0) + chr$(0) + chr$(0) + chr$(0) + chr$(0) + chr$(50) + chr$(64) then
    print "FAIL financial syd"
    end
end if
print "PASS financial"
end
