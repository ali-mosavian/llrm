' MKI$/MKL$ byte layout is witnessed before CVI/CVL decode it.
dim packedInteger as string * 2
dim packedLong as string * 4

packedInteger = mki$(4660)
if asc(mid$(packedInteger, 1, 1)) <> 52 or asc(mid$(packedInteger, 2, 1)) <> 18 then
    print "FAIL packing mki bytes"
    end
end if
if cvi(packedInteger) <> 4660 then
    print "FAIL packing cvi"
    end
end if
packedLong = mkl$(305419896)
if asc(mid$(packedLong, 1, 1)) <> 120 or asc(mid$(packedLong, 2, 1)) <> 86 then
    print "FAIL packing mkl low"
    end
end if
if asc(mid$(packedLong, 3, 1)) <> 52 or asc(mid$(packedLong, 4, 1)) <> 18 then
    print "FAIL packing mkl high"
    end
end if
if cvl(packedLong) <> 305419896 then
    print "FAIL packing cvl"
    end
end if
print "PASS packing"
end
