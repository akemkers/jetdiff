; Først, så alt mer spesifikt vinner
["(" ")" "[" "]" "{" "}" "," "." ";"] @punctuation
[
  "=" ":=" "==" "!=" "<" ">" "<=" ">=" "+" "-" "*" "/" "%" "&&" "||" "!" "&" "|" "^" "<<" ">>" "&^" "+=" "-=" "++" "--" "<-" "..."
] @operator

(field_identifier) @property
(call_expression function: (identifier) @function_call)
(call_expression function: (selector_expression field: (field_identifier) @function_call))

(type_identifier) @type

(function_declaration name: (identifier) @function)
(method_declaration name: (field_identifier) @function)

(comment) @comment

[
  "break" "case" "chan" "const" "continue" "default" "defer" "else" "fallthrough" "for" "func"
  "go" "goto" "if" "import" "interface" "map" "package" "range" "return" "select" "struct"
  "switch" "type" "var"
] @keyword
[(true) (false) (nil) (iota)] @keyword

[(interpreted_string_literal) (raw_string_literal) (rune_literal)] @string
(escape_sequence) @escape
[(int_literal) (float_literal) (imaginary_literal)] @number
