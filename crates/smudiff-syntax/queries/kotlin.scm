(class_parameter (simple_identifier) @property)
(class_body (property_declaration (variable_declaration (simple_identifier) @property)))
(navigation_suffix (simple_identifier) @property)
(call_expression (navigation_expression (navigation_suffix (simple_identifier) @plain) .))

(enum_entry (simple_identifier) @constant)
((simple_identifier) @constant
 (#match? @constant "^[A-Z][A-Z\\d_]+$"))

(function_declaration (simple_identifier) @function)

[(line_comment) (multiline_comment) (shebang_line)] @comment
((multiline_comment) @doc_comment (#match? @doc_comment "^/\\*\\*"))

[
  (class_modifier) (member_modifier) (function_modifier) (property_modifier) (platform_modifier)
  (variance_modifier) (parameter_modifier) (visibility_modifier) (reification_modifier)
  (inheritance_modifier) (jump_expression)
] @keyword
[
  "val" "var" "enum" "class" "object" "interface" "fun" "typealias" "import" "package" "if"
  "else" "when" "for" "do" "while" "try" "catch" "throw" "finally" "is" "in" "as" "as?"
] @keyword
[(null_literal) (boolean_literal) (this_expression) (super_expression)] @keyword

[(string_literal) (character_literal)] @string
[(interpolated_identifier) (interpolated_expression)] @plain
(character_escape_seq) @escape
[
  (integer_literal) (long_literal) (hex_literal) (bin_literal) (unsigned_literal) (real_literal)
] @number

(annotation) @metadata
(file_annotation) @metadata
