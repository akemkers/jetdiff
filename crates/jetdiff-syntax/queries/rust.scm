; Først, så alt mer spesifikt vinner
["(" ")" "[" "]" "{" "}" "," "." ";"] @punctuation
[
  "=" "==" "!=" "<" ">" "<=" ">=" "+" "-" "*" "/" "%" "&&" "||" "!" "&" "|" "^" "<<" ">>" "+=" "-=" "*=" "/=" "%=" "&=" "|=" "^=" "<<=" ">>=" "->" "=>" "::" ".." "..="
] @operator
"?" @q_operator

(field_identifier) @property

; Uten typeinfo gjettes stor forbokstav som struct, også traits
((identifier) @struct
 (#match? @struct "^[A-Z]"))
(type_identifier) @struct

((identifier) @constant
 (#match? @constant "^_*[A-Z][A-Z\\d_]+$"))

; Bare enumene fra prelude kan skilles fra structs
([(identifier) (type_identifier)] @enum
 (#any-of? @enum "Option" "Result" "Ordering" "Cow"))
(enum_item name: (type_identifier) @enum)

((identifier) @enum_variant
 (#any-of? @enum_variant "Some" "None" "Ok" "Err"))
(enum_variant name: (identifier) @enum_variant)
; `Type::Variant`, som `Status::Added`. Stor forbokstav på begge ledd skiller det fra
; `Type::function` og `Type::CONSTANT`
((scoped_identifier
  path: [(identifier) @struct (scoped_identifier name: (identifier) @struct)]
  name: (identifier) @enum_variant)
 (#match? @struct "^[A-Z]")
 (#match? @enum_variant "^[A-Z]")
 (#not-match? @enum_variant "^_*[A-Z][A-Z\\d_]+$"))
; `Type::Variant { .. }`. Ellers er `Type::Name` i en type en tilhørende type, som `Self::Item`
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

; Første ledd i en use-sti er en crate. Queries kan ikke matche rekursivt, så dybden skrives ut
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
; Utenfor use er første ledd som regel en modul, som `thread::scope`
((scoped_identifier path: (identifier) @crate)
 (#any-of? @crate "std" "core" "alloc"))

; Rust farger kall som deklarasjoner
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
(self) @self
; I en sti er `self` et nøkkelord, som `use self::a`
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
