(field_declaration declarator: (variable_declarator name: (identifier) @property))
(field_access field: (identifier) @property)
((identifier) @constant
 (#match? @constant "^_*[A-Z][A-Z\\d_]+$"))
(enum_constant name: (identifier) @constant)

(method_declaration name: (identifier) @function)
(constructor_declaration name: (identifier) @function)

[(line_comment) (block_comment)] @comment
((block_comment) @doc_comment (#match? @doc_comment "^/\\*\\*"))

[
  "abstract" "assert" "break" "case" "catch" "class" "continue" "default" "do" "else" "enum"
  "exports" "extends" "final" "finally" "for" "if" "implements" "import" "instanceof"
  "interface" "module" "native" "new" "non-sealed" "open" "opens" "package" "permits" "private"
  "protected" "provides" "public" "requires" "record" "return" "sealed" "static" "strictfp"
  "switch" "synchronized" "throw" "throws" "to" "transient" "transitive" "try" "uses"
  "volatile" "when" "while" "with" "yield"
] @keyword
[
  (boolean_type) (integral_type) (floating_point_type) (void_type) (this) (super) (true)
  (false) (null_literal)
] @keyword

[(character_literal) (string_literal)] @string
(escape_sequence) @escape
[
  (hex_integer_literal) (decimal_integer_literal) (octal_integer_literal) (binary_integer_literal)
  (decimal_floating_point_literal) (hex_floating_point_literal)
] @number

[(annotation) (marker_annotation)] @metadata
