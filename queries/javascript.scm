(function_declaration name: (identifier) @name.definition.function)
(generator_function_declaration name: (identifier) @name.definition.function)
(method_definition name: (property_identifier) @name.definition.method)
(variable_declarator name: (identifier) @name.definition.function
  value: [(arrow_function) (function_expression)])

(call_expression function: (identifier) @name.reference.call)
(call_expression function: (member_expression property: (property_identifier) @name.reference.call))
(new_expression constructor: (identifier) @name.reference.constructor)
(new_expression constructor: (member_expression property: (property_identifier) @name.reference.constructor))

(class_declaration name: (identifier) @name.definition.class)

; Only component names, never intrinsic HTML element names or closing tags.
((jsx_opening_element name: (identifier) @name.reference.jsx)
 (#match? @name.reference.jsx "^[A-Z]"))
((jsx_self_closing_element name: (identifier) @name.reference.jsx)
 (#match? @name.reference.jsx "^[A-Z]"))
((jsx_opening_element name: (member_expression property: (property_identifier) @name.reference.jsx))
 (#match? @name.reference.jsx "^[A-Z]"))
((jsx_self_closing_element name: (member_expression property: (property_identifier) @name.reference.jsx))
 (#match? @name.reference.jsx "^[A-Z]"))
