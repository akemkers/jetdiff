# One fold per run of unchanged lines

IntelliJ builds three nested folds for each run of unchanged lines, with 4, 8 and 16 lines of context, so each open shows a little more. jetdiff will build only one, with 4 lines of context. Nested folds made opening a fold unpredictable and the folding code harder to follow, and one open that shows everything is what we want.
