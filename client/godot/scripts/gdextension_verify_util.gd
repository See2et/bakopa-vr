extends RefCounted
class_name GDExtensionVerifyUtil

static func ensure_extension_loaded(extension_path: String) -> bool:
	var manager = Engine.get_singleton("GDExtensionManager")
	if manager == null:
		push_error("GDExtensionManager singleton not found")
		return false

	var abs_path = ProjectSettings.globalize_path(extension_path)
	var exists_res = FileAccess.file_exists(extension_path)
	var exists_abs = FileAccess.file_exists(abs_path)
	var loaded_res = manager.is_extension_loaded(extension_path)
	var loaded_abs = manager.is_extension_loaded(abs_path)
	var loaded = loaded_res or loaded_abs
	var loaded_list = manager.get_loaded_extensions()

	print("GDExtension check")
	print("path(res): ", extension_path)
	print("path(abs): ", abs_path)
	print("exists(res): ", exists_res, " exists(abs): ", exists_abs)
	print("loaded(res): ", loaded_res, " loaded(abs): ", loaded_abs)
	print("loaded_extensions: ", loaded_list)

	if loaded:
		print("OK: GDExtension is loaded")
		return true

	var status = manager.load_extension(abs_path)
	print("load_extension status: ", status)
	var loaded_after = manager.is_extension_loaded(abs_path)
	if loaded_after:
		print("OK: GDExtension loaded after manual load")
		return true

	push_error("NG: GDExtension not loaded")
	return false

static func initialize_openxr() -> bool:
	var xr_interface = XRServer.find_interface("OpenXR")
	if xr_interface == null:
		push_error("OpenXR interface not found: enable OpenXR in project settings")
		return false

	print("OpenXR interface found")
	var initialized = xr_interface.is_initialized()
	if not initialized:
		var init_result = xr_interface.initialize()
		print("OpenXR initialize result: ", init_result)

	var xr_ready = xr_interface.is_initialized()
	if xr_ready:
		XRServer.primary_interface = xr_interface
		print("XR primary interface set")

	print("OpenXR initialized: ", xr_ready)
	return xr_ready

static func configure_viewport_for_xr(host: Node, xr_ready: bool) -> void:
	var viewport = host.get_viewport()
	if viewport == null:
		push_error("Viewport not found")
		return

	viewport.use_xr = xr_ready
	print("Viewport use_xr set: ", viewport.use_xr)
