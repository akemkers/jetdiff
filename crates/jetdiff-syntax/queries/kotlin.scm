; First, so anything more specific wins
["(" ")" "[" "]" "{" "}" "," "." ";"] @punctuation
[
  "=" "==" "!=" "===" "!==" "<" ">" "<=" ">=" "+" "-" "*" "/" "%" "&&" "||" "!" "+=" "-=" "*=" "/=" "%=" "++" "--" "->" "::" ".." "?:" "?." "!!"
] @operator

(class_parameter (simple_identifier) @property)
(class_body (property_declaration (variable_declaration (simple_identifier) @property)))
(navigation_suffix (simple_identifier) @property)
(call_expression (navigation_expression (navigation_suffix (simple_identifier) @function_call) .))
(call_expression (simple_identifier) @function_call)
; A capital first letter is a constructor, which IntelliJ colors like the class
((call_expression (simple_identifier) @type)
 (#match? @type "^[A-Z]"))

(type_identifier) @type

(enum_entry (simple_identifier) @constant)
((simple_identifier) @constant
 (#match? @constant "^_*[A-Z][A-Z\\d_]+$"))

(function_declaration (simple_identifier) @function)

[(line_comment) (multiline_comment) (shebang_line)] @comment
((multiline_comment) @doc_comment (#match? @doc_comment "^/\\*\\*"))

[
  (class_modifier) (member_modifier) (function_modifier) (property_modifier) (platform_modifier)
  (variance_modifier) (parameter_modifier) (visibility_modifier) (reification_modifier)
  (inheritance_modifier)
] @keyword
[
  "val" "var" "enum" "class" "object" "interface" "fun" "typealias" "import" "package" "if"
  "else" "when" "for" "do" "while" "try" "catch" "throw" "finally" "is" "in" "as" "as?"
  "return" "return@" "break" "break@" "continue" "continue@"
] @keyword
[(null_literal) (boolean_literal) (this_expression) (super_expression)] @keyword

[(string_literal) (character_literal)] @string
[(interpolated_identifier) (interpolated_expression)] @plain
(character_escape_seq) @escape
[
  (integer_literal) (long_literal) (hex_literal) (bin_literal) (unsigned_literal) (real_literal)
] @number

(annotation) @metadata
; Otherwise the name is colored as a type
(annotation [
  (user_type (type_identifier) @metadata)
  (constructor_invocation (user_type (type_identifier) @metadata))
])
(file_annotation) @metadata
