[
  "FROM" "AS" "RUN" "CMD" "LABEL" "EXPOSE" "ENV" "ADD" "COPY" "ENTRYPOINT" "VOLUME" "USER"
  "WORKDIR" "ARG" "ONBUILD" "STOPSIGNAL" "HEALTHCHECK" "SHELL" "MAINTAINER" "CROSS_BUILD"
] @keyword

(comment) @comment

[(double_quoted_string) (single_quoted_string) (json_string) (heredoc_block)] @string
(escape_sequence) @escape

(expansion) @property

(expose_port) @number

["[" "]" ","] @punctuation
