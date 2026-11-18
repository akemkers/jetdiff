; First, so anything more specific wins
["(" ")" "[" "]" "{" "}" "," "." ":" ";"] @punctuation
[
  "=" "==" "~=" "<" ">" "<=" ">=" "+" "-" "*" "/" "//" "%" "^" "#" "&" "~" "|" "<<" ">>" ".."
] @operator

(dot_index_expression field: (identifier) @property)
(field name: (identifier) @property)

(function_call name: (identifier) @function_call)
(function_call name: (dot_index_expression field: (identifier) @function_call))
(function_call name: (method_index_expression method: (identifier) @function_call))

((identifier) @constant
 (#match? @constant "^_*[A-Z][A-Z\\d_]+$"))

(function_declaration name: (identifier) @function)
(function_declaration name: (dot_index_expression field: (identifier) @function))
(function_declaration name: (method_index_expression method: (identifier) @function))
; `M.f = function() end` and `{ f = function() end }`
(assignment_statement
  (variable_list . name: [(identifier) @function (dot_index_expression field: (identifier) @function)])
  (expression_list . value: (function_definition)))
(field name: (identifier) @function value: (function_definition))

((identifier) @self
 (#eq? @self "self"))

[(comment) (hash_bang_line)] @comment
; LuaDoc and EmmyLua annotations
((comment) @doc_comment (#match? @doc_comment "^---"))

[
  "and" "do" "else" "elseif" "end" "for" "function" "global" "goto" "if" "in" "local"
  "not" "or" "repeat" "return" "then" "until" "while"
] @keyword
[(nil) (true) (false) (vararg_expression) (break_statement)] @keyword
; `<const>` and `<close>`
(attribute "<" @keyword (identifier) @keyword ">" @keyword)

(string) @string
(escape_sequence) @escape
(number) @number
