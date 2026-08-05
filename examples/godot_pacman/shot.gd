# Dev-only: PACSHOT=1 saves /tmp/pacshot.png then quits; PACDIR=left/right/up/down
# holds that direction (to test movement headlessly). Inert without PACSHOT.
extends Node
var f := 0
func _ready():
	var d := OS.get_environment("PACDIR")
	if d != "":
		Input.action_press("ui_" + d)
func _process(_x):
	if OS.get_environment("PACSHOT") == "":
		return
	f += 1
	if f == 250:
		get_viewport().get_texture().get_image().save_png("/tmp/pacshot.png")
		get_tree().quit()
