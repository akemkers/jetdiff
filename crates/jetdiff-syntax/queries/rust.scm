; Først, så alt mer spesifikt vinner
["(" ")" "[" "]" "{" "}" "," "." ";"] @punctuation
[
  "=" "==" "!=" "<" ">" "<=" ">=" "+" "-" "*" "/" "%" "&&" "||" "!" "&" "|" "^" "<<" ">>" "+=" "-=" "*=" "/=" "%=" "&=" "|=" "^=" "<<=" ">>=" "->" "=>" "::" ".." "..=" "?"
] @operator

(field_identifier) @property

; Stor forbokstav er en type eller enum-variant, som `UiTheme::dark` og `Some`
((identifier) @type
 (#match? @type "^[A-Z]"))

((identifier) @constant
 (#match? @constant "^_*[A-Z][A-Z\\d_]+$"))

(type_identifier) @type

(call_expression function: (field_expression field: (field_identifier) @function_call))
((call_expression function: (scoped_identifier name: (identifier) @function_call))
 (#match? @function_call "^[a-z_]"))
((call_expression function: (identifier) @function_call)
 (#match? @function_call "^[a-z_]"))

(function_item name: (identifier) @function)
(function_signature_item name: (identifier) @function)
(macro_invocation macro: (identifier) @function "!" @function)

(line_comment) @comment
(block_comment) @comment
(line_comment (doc_comment)) @doc_comment
(block_comment (doc_comment)) @doc_comment
; Markøren i `///` er egne `/`-noder
(outer_doc_comment_marker _ @doc_comment)
(inner_doc_comment_marker _ @doc_comment)

[
  "as" "async" "await" "break" "const" "continue" "default" "dyn" "else" "enum" "extern" "fn"
  "for" "gen" "if" "impl" "in" "let" "loop" "macro_rules!" "match" "mod" "move" "pub" "raw"
  "ref" "return" "static" "struct" "trait" "type" "union" "unsafe" "use" "where" "while" "yield"
] @keyword
(crate) @keyword
(super) @keyword
(self) @keyword
(mutable_specifier) @keyword
(primitive_type) @keyword
(boolean_literal) @keyword

[(char_literal) (string_literal) (raw_string_literal)] @string
(escape_sequence) @escape
[(integer_literal) (float_literal)] @number

(attribute_item) @metadata
(inner_attribute_item) @metadata
; Hele attributtet i én farge, også navn og tegn inni, som `#[derive(Debug)]`
(attribute_item ["#" "[" "]"] @metadata)
(inner_attribute_item ["#" "!" "[" "]"] @metadata)
(attribute (identifier) @metadata)
(attribute (scoped_identifier (identifier) @metadata))
; Queries kan ikke matche rekursivt, så dybden skrives ut
(attribute (token_tree ["(" ")" "," "=" "::"] @metadata))
(attribute (token_tree (identifier) @metadata))
(attribute (token_tree (token_tree ["(" ")" "," "=" "::"] @metadata)))
(attribute (token_tree (token_tree (identifier) @metadata)))
(attribute (token_tree (token_tree (token_tree ["(" ")" "," "=" "::"] @metadata))))
(attribute (token_tree (token_tree (token_tree (identifier) @metadata))))
