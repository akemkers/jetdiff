; First, so anything more specific wins
["(" ")" "[" "]" "{" "}" "," "." ":" ";"] @punctuation
[
  "=" ":=" "==" "!=" "<" ">" "<=" ">=" "<>" "+" "-" "*" "**" "/" "//" "%" "@" "&" "|" "^" "~"
  "<<" ">>" "+=" "-=" "*=" "**=" "/=" "//=" "%=" "@=" "&=" "|=" "^=" "<<=" ">>=" "->"
] @operator

(call function: (identifier) @function_call)
(call function: (attribute attribute: (identifier) @function_call))

(function_definition name: (identifier) @function)
(class_definition name: (identifier) @type)

((identifier) @self
 (#match? @self "^(self|cls)$"))

(comment) @comment

[
  "and" "as" "assert" "async" "await" "break" "case" "class" "continue" "def" "del" "elif" "else"
  "except" "finally" "for" "from" "global" "if" "import" "in" "is" "lambda" "match"
  "nonlocal" "not" "or" "pass" "raise" "return" "try" "type" "while" "with" "yield"
] @keyword
[(true) (false) (none) (ellipsis)] @keyword

(string) @string
; The first string in a module, class or function is its docstring, also after a shebang
(module . (comment)* . (expression_statement [(string) @doc_comment (concatenated_string (string) @doc_comment) @doc_comment]))
(function_definition
  body: (block . (expression_statement [(string) @doc_comment (concatenated_string (string) @doc_comment) @doc_comment])))
(class_definition
  body: (block . (expression_statement [(string) @doc_comment (concatenated_string (string) @doc_comment) @doc_comment])))
(interpolation) @plain
; `!r` and `:>10` belong to the string, but `{width}` inside the format is code
[(type_conversion) (format_specifier)] @string
(format_expression) @plain
[(escape_sequence) (escape_interpolation)] @escape
[(integer) (float)] @number

; Only `@` and the name; the arguments are colored as code
(decorator "@" @metadata)
(decorator [(identifier) (attribute)] @metadata)
(decorator (call function: (_) @metadata))
; Otherwise the name is colored as a call and the dots as punctuation, up to `@a.b.c.d`
(decorator (call function: (attribute attribute: (identifier) @metadata)))
(decorator [
  (attribute "." @metadata)
  (attribute object: (attribute "." @metadata))
  (attribute object: (attribute object: (attribute "." @metadata)))
  (call function: (attribute "." @metadata))
  (call function: (attribute object: (attribute "." @metadata)))
  (call function: (attribute object: (attribute object: (attribute "." @metadata))))
])
