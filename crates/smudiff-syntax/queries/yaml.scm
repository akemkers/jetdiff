[(double_quote_scalar) (single_quote_scalar) (block_scalar) (string_scalar)] @string
[(integer_scalar) (float_scalar)] @number
[(boolean_scalar) (null_scalar)] @keyword
(comment) @comment

; `YAML_SCALAR_KEY` arver `DEFAULT_KEYWORD`
(block_mapping_pair key: (flow_node (_) @keyword))
(block_mapping_pair key: (flow_node (plain_scalar (string_scalar) @keyword)))
(flow_pair key: (flow_node (_) @keyword))
(flow_pair key: (flow_node (plain_scalar (string_scalar) @keyword)))
