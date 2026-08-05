# Dev-only screenshot helper: with PACSHOT=1 in the environment it saves a frame
# to /tmp/pacshot.png and quits; otherwise it's inert, so the game plays normally.
extends Node
var f := 0
func _process(_d):
	if OS.get_environment("PACSHOT") == "":
		return
	f += 1
	if f == 8:
		get_viewport().get_texture().get_image().save_png("/tmp/pacshot.png")
		get_tree().quit()
