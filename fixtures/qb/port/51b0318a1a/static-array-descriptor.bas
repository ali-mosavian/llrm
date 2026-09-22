option explicit

declare function touch ( values() as long ) as integer

'$static
dim shared values(3) as long

dim result as integer

result = touch(values())
