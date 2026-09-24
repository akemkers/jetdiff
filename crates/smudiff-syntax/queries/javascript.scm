(property_identifier) @property
(call_expression function: (member_expression property: (property_identifier) @plain))

([(identifier) (shorthand_property_identifier) (shorthand_property_identifier_pattern)] @constant
 (#match? @constant "^[A-Z_][A-Z\\d_]+$"))

(function_declaration name: (identifier) @function)
(function_expression name: (identifier) @function)
(generator_function_declaration name: (identifier) @function)
(method_definition name: (property_identifier) @function)
(variable_declarator name: (identifier) @function value: [(function_expression) (arrow_function)])
(pair key: (property_identifier) @function value: [(function_expression) (arrow_function)])

(comment) @comment
((comment) @doc_comment (#match? @doc_comment "^/\\*\\*"))

[
  "as" "async" "await" "break" "case" "catch" "class" "const" "continue" "debugger" "default"
  "delete" "do" "else" "export" "extends" "finally" "for" "from" "function" "get" "if" "import"
  "in" "instanceof" "let" "new" "of" "return" "set" "static" "switch" "target" "throw" "try"
  "typeof" "var" "void" "while" "with" "yield"
] @keyword
[(true) (false) (null) (undefined) (this) (super)] @keyword

[(string) (template_string) (regex)] @string
(template_substitution) @plain
(escape_sequence) @escape
(number) @number

(decorator) @metadata

(jsx_opening_element name: (identifier) @tag (#match? @tag "^[a-z][^.]*$"))
(jsx_closing_element name: (identifier) @tag (#match? @tag "^[a-z][^.]*$"))
(jsx_self_closing_element name: (identifier) @tag (#match? @tag "^[a-z][^.]*$"))
(jsx_attribute (property_identifier) @attribute)
