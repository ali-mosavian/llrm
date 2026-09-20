' /MBF changes the MKS$/CVS family to Microsoft Binary Format routines.
dim sourceValue as single
dim sourceDouble as double
dim encodedValue as string
dim encodedDouble as string
dim restoredValue as single
dim restoredDouble as double

read sourceValue, sourceDouble
encodedValue = mks$(sourceValue)
restoredValue = cvs(encodedValue)
if encodedValue <> chr$(0) + chr$(0) + chr$(64) + chr$(129) then
    print "FAIL pds-mbf single-bytes"
    end
end if
if restoredValue <> 1.5 then
    print "FAIL pds-mbf single-value"
    end
end if
encodedDouble = mkd$(sourceDouble)
restoredDouble = cvd(encodedDouble)
if encodedDouble <> chr$(248) + chr$(255) + chr$(255) + chr$(255) + chr$(255) + chr$(255) + chr$(63) + chr$(129) then
    print "FAIL pds-mbf double-bytes"
    end
end if
if restoredDouble <> sourceDouble then
    print "FAIL pds-mbf double-value"
    end
end if
print "PASS pds-mbf"
end

data 1.5, 1.5
