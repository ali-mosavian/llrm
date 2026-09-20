option explicit

declare function waitKey (keyDown as integer) as integer

end

function waitKey (keyDown as integer) as integer
    if keyDown = 0 then
        waitKey = 0
        exit function
    end if

    do
    loop while keyDown

    waitKey = -1
end function
