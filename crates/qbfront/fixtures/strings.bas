type Texture
    name as string * 16
end type

dim texture as Texture
dim suffix as string
dim oneChar as string * 1

suffix = mid$(rtrim$(texture.name), 3)
oneChar = mid$(suffix, 1, 1)
suffix = chr$(65)

if oneChar = "A" then suffix = "matched"
suffix = suffix + oneChar + "!"
