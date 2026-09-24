; Generelle mønstre først. tree-sitter-highlight lar det siste treffet på en node vinne.

(field_identifier) @property

((identifier) @constant
 (#match? @constant "^[A-Z][A-Z\\d_]+$"))

(function_item name: (identifier) @function)
(function_signature_item name: (identifier) @function)
(macro_invocation macro: (identifier) @function "!" @function)

(line_comment) @comment
(block_comment) @comment
(line_comment (doc_comment)) @doc_comment
(block_comment (doc_comment)) @doc_comment

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
