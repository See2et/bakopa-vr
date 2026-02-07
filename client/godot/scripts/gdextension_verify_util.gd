extends RefCounted
class_name GDExtensionVerifyUtil

static func ensure_extension_loaded(extension_path: String) -> bool:
	var debug_build = Engine.is_debug_build()
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

	if debug_build:
		print("GDExtension check")
		print("path(res): ", extension_path)
		print("path(abs): ", abs_path)
		print("exists(res): ", exists_res, " exists(abs): ", exists_abs)
		print("loaded(res): ", loaded_res, " loaded(abs): ", loaded_abs)
		print("loaded_extensions: ", loaded_list)

	if loaded:
		if debug_build:
			print("OK: GDExtension is loaded")
		return true

	var status = manager.load_extension(abs_path)
	if debug_build:
		print("load_extension status: ", status)
	if status != GDExtensionManager.LOAD_STATUS_OK and status != GDExtensionManager.LOAD_STATUS_ALREADY_LOADED:
		push_error(
			"NG: load_extension failed or requires restart"
			+ " status=" + _load_status_to_string(status)
			+ " path=" + abs_path
		)
		return false

	var loaded_after = manager.is_extension_loaded(abs_path)
	if loaded_after:
		if debug_build:
			print("OK: GDExtension loaded after manual load")
		return true

	push_error("NG: GDExtension not loaded")
	return false

static func initialize_openxr() -> bool:
	var debug_build = Engine.is_debug_build()
	var xr_interface = XRServer.find_interface("OpenXR")
	if xr_interface == null:
		push_error("OpenXR interface not found: enable OpenXR in project settings")
		return false

	if debug_build:
		print("OpenXR interface found")
	var initialized = xr_interface.is_initialized()
	if not initialized:
		var init_result = xr_interface.initialize()
		if debug_build:
			print("OpenXR initialize result: ", init_result)
		if not init_result:
			push_error("OpenXR initialize() failed: returned false")
			return false

	var xr_ready = xr_interface.is_initialized()
	if xr_ready:
		XRServer.primary_interface = xr_interface
		if debug_build:
			print("XR primary interface set")

	if debug_build:
		print("OpenXR initialized: ", xr_ready)
	return xr_ready

static func configure_viewport_for_xr(host: Node, xr_ready: bool) -> void:
	var debug_build = Engine.is_debug_build()
	var viewport = host.get_viewport()
	if viewport == null:
		push_error("Viewport not found")
		return

	viewport.use_xr = xr_ready
	if debug_build:
		print("Viewport use_xr set: ", viewport.use_xr)

static func _load_status_to_string(status: int) -> String:
	match status:
		GDExtensionManager.LOAD_STATUS_OK:
			return "LOAD_STATUS_OK"
		GDExtensionManager.LOAD_STATUS_FAILED:
			return "LOAD_STATUS_FAILED"
		GDExtensionManager.LOAD_STATUS_ALREADY_LOADED:
			return "LOAD_STATUS_ALREADY_LOADED"
		GDExtensionManager.LOAD_STATUS_NOT_LOADED:
			return "LOAD_STATUS_NOT_LOADED"
		GDExtensionManager.LOAD_STATUS_NEEDS_RESTART:
			return "LOAD_STATUS_NEEDS_RESTART"
		_:
			return "UNKNOWN_STATUS(%s)" % status
