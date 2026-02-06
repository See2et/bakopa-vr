extends Node

@export var extension_path: String = "res://client_core.gdextension"

func _ready() -> void:
	if not GDExtensionVerifyUtil.ensure_extension_loaded(extension_path):
		return

	var xr_ready = GDExtensionVerifyUtil.initialize_openxr()
	GDExtensionVerifyUtil.configure_viewport_for_xr(self, xr_ready)
