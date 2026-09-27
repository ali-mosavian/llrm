' Text/numeric conversions use literal expected spellings, not inverse pairs.
dim decimalText as string

read decimalText
if abs(val(decimalText) + 123.5) > .000000000001 then
    print "FAIL textconv val"
    end
end if
if str$(42) <> " 42" or str$(-42) <> "-42" then
    print "FAIL textconv str"
    end
end if
if hex$(4660) <> "1234" then
    print "FAIL textconv hex"
    end
end if
if oct$(511) <> "777" then
    print "FAIL textconv oct"
    end
end if
print "PASS textconv"
end

data "  -123.5xyz"
