' PDS CURRENCY has four decimal places.
dim firstAmount as currency
dim secondAmount as currency
dim passed as integer
dim packedAmount as string * 8

firstAmount = ccur(123.45)
secondAmount = firstAmount + ccur(.55)
passed = secondAmount = ccur(124)
packedAmount = mkc$(firstAmount)
passed = passed and len(packedAmount) = 8
passed = passed and asc(mid$(packedAmount, 1, 1)) = 68
passed = passed and asc(mid$(packedAmount, 2, 1)) = 214
passed = passed and asc(mid$(packedAmount, 3, 1)) = 18
passed = passed and asc(mid$(packedAmount, 4, 1)) = 0
passed = passed and cvc(packedAmount) = firstAmount

if passed then
    print "PASS pds-currency"
else
    print "FAIL pds-currency arithmetic"
end if
end
