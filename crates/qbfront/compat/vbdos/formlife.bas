option explicit

sub Form_Load ()
    frmCompat.Caption = "loaded"
end sub

sub Form_Resize ()
    frmCompat.Refresh
end sub

sub Form_Paint ()
    frmCompat.Print "paint"
end sub

sub Form_MouseDown (buttonValue as integer, shiftValue as integer, posX as single, posY as single)
    frmCompat.CurrentX = posX
end sub

sub Form_Unload (cancelValue as integer)
    cancelValue = 0
end sub
