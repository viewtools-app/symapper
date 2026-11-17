; Declaration keywords distinguish the shared class_declaration node.
(class_declaration declaration_kind: "class" name: (type_identifier) @name.definition.class)
(class_declaration declaration_kind: "struct" name: (type_identifier) @name.definition.struct)
(class_declaration declaration_kind: "enum" name: (type_identifier) @name.definition.enum)
(class_declaration declaration_kind: "actor" name: (type_identifier) @name.definition.actor)
(class_declaration declaration_kind: "extension"
  name: (user_type (type_identifier) @name.definition.extension))
(protocol_declaration name: (type_identifier) @name.definition.protocol)
(function_declaration name: (simple_identifier) @name.definition.function)
(protocol_function_declaration name: (simple_identifier) @name.definition.function)
(init_declaration name: "init" @name.definition.initializer)
(property_declaration name: (pattern bound_identifier: (simple_identifier) @name.definition.property))
(typealias_declaration name: (type_identifier) @name.definition.type)
(enum_entry name: (simple_identifier) @name.definition.variant)

(call_expression . (simple_identifier) @name.reference.call)
(call_expression . (navigation_expression
  suffix: (navigation_suffix suffix: (simple_identifier) @name.reference.call)))
(inheritance_specifier inherits_from: (user_type (type_identifier) @name.reference.type))
(type_annotation (user_type (type_identifier) @name.reference.type))
(parameter (user_type (type_identifier) @name.reference.type))
; Use child shapes: Swift field aliases differ between metadata and parsed trees.
(function_declaration (user_type (type_identifier) @name.reference.type))
(protocol_function_declaration (user_type (type_identifier) @name.reference.type))
; This grammar labels the alias target as name as well.
(typealias_declaration (user_type (type_identifier) @name.reference.type))
(type_arguments (user_type (type_identifier) @name.reference.type))
(array_type (user_type (type_identifier) @name.reference.type))
(dictionary_type (user_type (type_identifier) @name.reference.type))
(optional_type (user_type (type_identifier) @name.reference.type))
(tuple_type_item (user_type (type_identifier) @name.reference.type))
(function_type (user_type (type_identifier) @name.reference.type))
(protocol_composition_type (user_type (type_identifier) @name.reference.type))
