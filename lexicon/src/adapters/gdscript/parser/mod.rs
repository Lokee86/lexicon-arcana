mod calls;
mod declarations;
mod imports;

pub use calls::{find_calls, terminal_call};
pub use declarations::parse_file;
pub use imports::{find_imports, normalize_import_path, project_resource_path};

use super::model::{ParsedFile, Statement};

pub fn declaration_for_statement<'a>(
    file: &'a ParsedFile,
    statement: &Statement,
) -> Option<&'a super::model::Declaration> {
    file.declarations.iter().find(|declaration| {
        declaration.span.start_line == statement.start.line
            && declaration.span.start_column == statement.start.column
    })
}

pub fn is_declaration_keyword(value: &str) -> bool {
    matches!(
        value,
        "class_name" | "class" | "func" | "signal" | "const" | "var" | "extends"
    )
}

pub fn is_call_keyword(value: &str) -> bool {
    matches!(
        value,
        "if" | "elif"
            | "while"
            | "for"
            | "match"
            | "func"
            | "signal"
            | "class"
            | "class_name"
            | "extends"
            | "var"
            | "const"
            | "return"
            | "await"
            | "yield"
            | "and"
            | "or"
            | "not"
    )
}

pub fn is_builtin(value: &str) -> bool {
    matches!(
        value,
        "Node"
            | "Node2D"
            | "Node3D"
            | "Object"
            | "RefCounted"
            | "Resource"
            | "Control"
            | "CanvasItem"
            | "CharacterBody2D"
            | "CharacterBody3D"
            | "Area2D"
            | "Area3D"
            | "Sprite2D"
            | "Sprite3D"
            | "PackedScene"
            | "SceneTree"
            | "Engine"
            | "ProjectSettings"
            | "Input"
            | "Time"
            | "OS"
            | "FileAccess"
            | "DirAccess"
            | "JSON"
            | "Marshalls"
            | "Geometry2D"
            | "Geometry3D"
            | "PhysicsServer2D"
            | "PhysicsServer3D"
            | "RenderingServer"
            | "AudioServer"
            | "DisplayServer"
            | "ClassDB"
            | "ResourceLoader"
            | "ResourceSaver"
            | "String"
            | "StringName"
            | "Vector2"
            | "Vector2i"
            | "Vector3"
            | "Vector3i"
            | "Vector4"
            | "Vector4i"
            | "Color"
            | "Transform2D"
            | "Transform3D"
            | "Basis"
            | "Quaternion"
            | "Rect2"
            | "Rect2i"
            | "Array"
            | "Dictionary"
            | "Callable"
            | "Signal"
            | "print"
            | "prints"
            | "print_debug"
            | "print_stack"
            | "push_error"
            | "push_warning"
            | "str"
            | "len"
            | "range"
            | "is_instance_valid"
            | "instance_from_id"
            | "is_same"
            | "typeof"
            | "type_string"
            | "preload"
            | "load"
            | "abs"
            | "min"
            | "max"
            | "clamp"
            | "lerp"
            | "inverse_lerp"
            | "remap"
            | "move_toward"
            | "snapped"
            | "wrapi"
            | "wrapf"
            | "floor"
            | "ceil"
            | "round"
            | "sqrt"
            | "pow"
            | "sin"
            | "cos"
            | "tan"
            | "deg_to_rad"
            | "rad_to_deg"
            | "randf"
            | "randi"
            | "randf_range"
            | "randi_range"
            | "assert"
            | "int"
            | "float"
            | "bool"
            | "NodePath"
            | "PackedByteArray"
            | "PackedInt32Array"
            | "PackedInt64Array"
            | "PackedFloat32Array"
            | "PackedFloat64Array"
            | "PackedStringArray"
            | "weakref"
            | "error_string"
            | "inst_to_dict"
            | "is_instance_id_valid"
            | "printraw"
    )
}
