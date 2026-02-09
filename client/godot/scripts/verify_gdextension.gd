extends Node3D

@export var extension_path: String = "res://client_core.gdextension"
@export var bridge_node_path: NodePath = NodePath("SuteraClientBridge")

var _bridge: Node = null

func _log_startup_stage(stage: String, mode: String, detail: String) -> void:
	var library_path := ProjectSettings.globalize_path(extension_path)
	print("stage=%s mode=%s library_path=%s detail=%s" % [stage, mode, library_path, detail])

func _ready() -> void:
	var mode := "desktop"
	if GDExtensionVerifyUtil.should_initialize_openxr():
		mode = "vr"
	_log_startup_stage("extension_check", mode, "begin")
	if not GDExtensionVerifyUtil.ensure_extension_loaded(extension_path):
		_log_startup_stage("extension_load_failed", mode, "ensure_extension_loaded returned false")
		return
	_log_startup_stage("extension_loaded", mode, "ok")

	var xr_ready = false
	if GDExtensionVerifyUtil.should_initialize_openxr():
		xr_ready = GDExtensionVerifyUtil.initialize_openxr()
		_log_startup_stage("openxr_init", mode, "xr_ready=%s" % xr_ready)
	else:
		print("Desktop mode active: skipped OpenXR initialization")
		_log_startup_stage("openxr_skip", mode, "desktop_mode")
	GDExtensionVerifyUtil.configure_viewport_for_xr(self, xr_ready)
	_bridge = get_node_or_null(bridge_node_path)
	if _bridge == null:
		_log_startup_stage("bridge_missing", mode, "node_not_found")
		push_error("SuteraClientBridge node not found: %s" % bridge_node_path)
		return
	if _bridge.has_method("on_start"):
		var started: bool = bool(_bridge.call("on_start"))
		if not started:
			_log_startup_stage("bridge_start", mode, "started=false")
			push_error("SuteraClientBridge.on_start returned false")
		else:
			_log_startup_stage("bridge_start", mode, "started=true")
	else:
		_log_startup_stage("bridge_start", mode, "on_start_not_found")

func _process(_delta: float) -> void:
	if _bridge != null and _bridge.has_method("on_frame"):
		_bridge.call("on_frame")

func _unhandled_input(event: InputEvent) -> void:
	if _bridge == null or not _bridge.has_method("push_input_event"):
		return
	_bridge.call("push_input_event", event)
	if event is InputEventMouseMotion:
		_forward_mouse_motion_as_actions(event)

func _forward_mouse_motion_as_actions(event: InputEventMouseMotion) -> void:
	var rel := event.relative
	if rel.x < 0.0:
		_push_look_action("look_left")
	elif rel.x > 0.0:
		_push_look_action("look_right")
	if rel.y < 0.0:
		_push_look_action("look_up")
	elif rel.y > 0.0:
		_push_look_action("look_down")

func _push_look_action(action_name: String) -> void:
	var action := InputEventAction.new()
	action.action = action_name
	action.pressed = true
	action.strength = 1.0
	_bridge.call("push_input_event", action)
