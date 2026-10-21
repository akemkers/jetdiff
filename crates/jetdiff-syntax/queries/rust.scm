; First, so anything more specific wins
["(" ")" "[" "]" "{" "}" "," "." ";"] @punctuation
[
  "=" "==" "!=" "<" ">" "<=" ">=" "+" "-" "*" "/" "%" "&&" "||" "!" "&" "|" "^" "<<" ">>" "+=" "-=" "*=" "/=" "%=" "&=" "|=" "^=" "<<=" ">>=" "->" "=>" "::" ".." "..="
] @operator
"?" @q_operator

(field_identifier) @property

; Without type info, a capital first letter is guessed to be a struct, traits too
((identifier) @struct
 (#match? @struct "^[A-Z]"))
(type_identifier) @struct

((identifier) @constant
 (#match? @constant "^_*[A-Z][A-Z\\d_]+$"))

; Only the enums from the prelude can be told apart from structs
([(identifier) (type_identifier)] @enum
 (#any-of? @enum "Option" "Result" "Ordering" "Cow"))
(enum_item name: (type_identifier) @enum)

((identifier) @enum_variant
 (#any-of? @enum_variant "Some" "None" "Ok" "Err"))
(enum_variant name: (identifier) @enum_variant)
; `Type::Variant`, like `Status::Added`. A capital first letter on both segments tells it apart from
; `Type::function` and `Type::CONSTANT`
((scoped_identifier
  path: [(identifier) @struct (scoped_identifier name: (identifier) @struct)]
  name: (identifier) @enum_variant)
 (#match? @struct "^[A-Z]")
 (#match? @enum_variant "^[A-Z]")
 (#not-match? @enum_variant "^_*[A-Z][A-Z\\d_]+$"))
; `Type::Variant { .. }`. Otherwise `Type::Name` in a type is an associated type, like `Self::Item`
((struct_expression
  name: (scoped_type_identifier
    path: (identifier) @struct
    name: (type_identifier) @enum_variant))
 (#match? @struct "^[A-Z]"))
((struct_pattern
  type: (scoped_type_identifier
    path: (identifier) @struct
    name: (type_identifier) @enum_variant))
 (#match? @struct "^[A-Z]"))

; The first segment of a use path is a crate. Queries can't match recursively, so the depth is written out
(use_declaration argument: (scoped_identifier path: (identifier) @crate))
(use_declaration argument: (scoped_identifier path: (scoped_identifier path: (identifier) @crate)))
(use_declaration argument: (scoped_identifier path: (scoped_identifier path: (scoped_identifier path: (identifier) @crate))))
(use_declaration argument: (scoped_use_list path: (identifier) @crate))
(use_declaration argument: (scoped_use_list path: (scoped_identifier path: (identifier) @crate)))
(use_declaration argument: (scoped_use_list path: (scoped_identifier path: (scoped_identifier path: (identifier) @crate))))
(use_declaration argument: (use_wildcard (identifier) @crate))
(use_declaration argument: (use_wildcard (scoped_identifier path: (identifier) @crate)))
(use_declaration argument: (use_wildcard (scoped_identifier path: (scoped_identifier path: (identifier) @crate))))
(extern_crate_declaration name: (identifier) @crate)
; Outside use, the first segment is usually a module, like `thread::scope`
((scoped_identifier path: (identifier) @crate)
 (#any-of? @crate "std" "core" "alloc"))

; Rust colors calls like declarations
((call_expression function: (identifier) @function)
 (#match? @function "^[a-z_]"))
((call_expression function: (scoped_identifier name: (identifier) @function))
 (#match? @function "^[a-z_]"))
(call_expression function: (field_expression field: (field_identifier) @function))
((generic_function function: (identifier) @function)
 (#match? @function "^[a-z_]"))
((generic_function function: (scoped_identifier name: (identifier) @function))
 (#match? @function "^[a-z_]"))
(generic_function function: (field_expression field: (field_identifier) @function))

(function_item name: (identifier) @function)
(function_signature_item name: (identifier) @function)
(macro_invocation macro: (identifier) @macro "!" @macro)
(macro_invocation macro: (scoped_identifier name: (identifier) @macro) "!" @macro)
(macro_definition name: (identifier) @macro)

(line_comment) @comment
(block_comment) @comment
(line_comment (doc_comment)) @doc_comment
(block_comment (doc_comment)) @doc_comment
; The marker in `///` is its own `/` nodes
(outer_doc_comment_marker _ @doc_comment)
(inner_doc_comment_marker _ @doc_comment)

[
  "as" "async" "await" "break" "const" "continue" "default" "dyn" "else" "enum" "extern" "fn"
  "for" "gen" "if" "impl" "in" "let" "loop" "macro_rules!" "match" "mod" "move" "pub" "raw"
  "ref" "return" "static" "struct" "trait" "type" "union" "unsafe" "use" "where" "while" "yield"
] @keyword
(crate) @keyword
(super) @keyword
(self) @self
; In a path, `self` is a keyword, like `use self::a`
(scoped_identifier path: (self) @keyword)
(scoped_use_list path: (self) @keyword)
(use_list (self) @keyword)
([(identifier) (type_identifier)] @keyword
 (#eq? @keyword "Self"))
(mutable_specifier) @keyword
(primitive_type) @keyword
(boolean_literal) @keyword

[(char_literal) (string_literal) (raw_string_literal)] @string
(escape_sequence) @escape
[(integer_literal) (float_literal)] @number

(attribute_item) @metadata
(inner_attribute_item) @metadata
; The whole attribute in one color, including the name and characters inside, like `#[derive(Debug)]`
(attribute_item ["#" "[" "]"] @metadata)
(inner_attribute_item ["#" "!" "[" "]"] @metadata)
(attribute (identifier) @metadata)
(attribute (scoped_identifier (identifier) @metadata))
; Queries can't match recursively, so the depth is written out
(attribute (token_tree ["(" ")" "," "=" "::"] @metadata))
(attribute (token_tree (identifier) @metadata))
(attribute (token_tree (token_tree ["(" ")" "," "=" "::"] @metadata)))
(attribute (token_tree (token_tree (identifier) @metadata)))
(attribute (token_tree (token_tree (token_tree ["(" ")" "," "=" "::"] @metadata))))
(attribute (token_tree (token_tree (token_tree (identifier) @metadata))))
