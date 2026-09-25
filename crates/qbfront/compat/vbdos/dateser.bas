option explicit

dim serialDate as double
dim serialTime as double


serialDate = dateserial(1992, 2, 29)
serialTime = timeserial(12, 34, 56)

if dateserial(1899, 12, 30) <> 0 or timeserial(6, 0, 0) <> .25 then
    print "FAIL date_serial_surface epoch"
    end
end if

if year(serialDate) <> 1992 or month(serialDate) <> 2 or day(serialDate) <> 29 then
    print "FAIL date_serial_surface date"
    end
end if

if hour(serialTime) <> 12 or minute(serialTime) <> 34 or second(serialTime) <> 56 then
    print "FAIL date_serial_surface time"
    end
end if

if datevalue("2/29/1992") <> serialDate or timevalue("12:34:56") <> serialTime then
    print "FAIL date_serial_surface parse"
    end
end if
if datevalue("12/30/1899") <> 0 or timevalue("6:00:00") <> .25 then
    print "FAIL date_serial_surface anchors"
    end
end if

print "PASS date_serial_surface"
