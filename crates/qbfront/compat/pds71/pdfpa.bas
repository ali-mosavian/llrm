' /FPa requires the alternate floating-point library selected at link time.
dim cosineInput as double
dim sineInput as double
dim logInput as double
dim cosineValue as double
dim sineValue as double
dim logValue as double

read cosineInput, sineInput, logInput
cosineValue = cos(cosineInput)
sineValue = sin(sineInput)
logValue = log(logInput)
if mkd$(cosineValue) <> chr$(140) + chr$(6) + chr$(181) + chr$(15) + chr$(40) + chr$(74) + chr$(225) + chr$(63) then
    print "FAIL pds-fpa cosine"
    end
end if
if mkd$(sineValue) <> chr$(240) + chr$(5) + chr$(75) + chr$(116) + chr$(232) + chr$(174) + chr$(222) + chr$(63) then
    print "FAIL pds-fpa sine"
    end
end if
if mkd$(logValue) <> chr$(239) + chr$(57) + chr$(250) + chr$(254) + chr$(66) + chr$(46) + chr$(230) + chr$(63) then
    print "FAIL pds-fpa log"
    end
end if
print "PASS pds-fpa"
end

data 1, .5, 2
