option explicit

declare function secondItem (items() as string) as string
declare function isYes (items() as string) as integer

dim items(2) as string

items(2) = "yes"
print isYes(items())

function secondItem (items() as string) as string
    secondItem = items(2)
end function

function isYes (items() as string) as integer
    dim value as string

    value = secondItem(items())
    if value = "yes" then
        isYes = -1
    else
        isYes = 0
    end if
end function
