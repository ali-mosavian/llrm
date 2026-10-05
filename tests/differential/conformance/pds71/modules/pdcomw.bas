common shared /memoryPool/ value&

sub changeValue (leftValue as long, rightValue as long)
    value& = value& + leftValue + rightValue
end sub
