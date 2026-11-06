//! Syntax highlighting with tree-sitter. Gives tokens with byte ranges for a whole file, so
//! deleted lines get the right syntax. The tokens correspond to IntelliJ's `DEFAULT_*` keys.

use std::ops::Range;
use std::sync::OnceLock;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Go,
    Java,
    JavaScript,
    Json,
    Kotlin,
    Markdown,
    Python,
    Rust,
    Toml,
    Tsx,
    TypeScript,
    Xml,
    Yaml,
}

/// The same order as the enum, so `language as usize` is the index.
const LANGUAGES: [Language; 13] = [
    Language::Go,
    Language::Java,
    Language::JavaScript,
    Language::Json,
    Language::Kotlin,
    Language::Markdown,
    Language::Python,
    Language::Rust,
    Language::Toml,
    Language::Tsx,
    Language::TypeScript,
    Language::Xml,
    Language::Yaml,
];

impl Language {
    pub fn from_path(path: &str) -> Option<Self> {
        let extension = path.rsplit_once('.')?.1;
        Some(match extension.to_ascii_lowercase().as_str() {
            "go" => Self::Go,
            "java" => Self::Java,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "json" => Self::Json,
            "kt" | "kts" => Self::Kotlin,
            "md" | "markdown" => Self::Markdown,
            "py" | "pyi" | "pyw" => Self::Python,
            "rs" => Self::Rust,
            "toml" => Self::Toml,
            "tsx" => Self::Tsx,
            "ts" | "mts" | "cts" => Self::TypeScript,
            "xml" => Self::Xml,
            "yaml" | "yml" => Self::Yaml,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Keyword,
    String,
    Escape,
    Number,
    Comment,
    DocComment,
    Function,
    FunctionCall,
    /// Class and type names, `DEFAULT_CLASS_NAME` and `DEFAULT_CLASS_REFERENCE`.
    Type,
    Constant,
    Property,
    Metadata,
    Tag,
    Attribute,
    Heading,
    Link,
    Strong,
    Emphasis,
    /// `DEFAULT_PARENTHS`, `DEFAULT_BRACES`, `DEFAULT_COMMA` and the like.
    Punctuation,
    /// `DEFAULT_OPERATION_SIGN`.
    Operator,
    /// Rust has its own keys with a color where `DEFAULT_*` is plain text, like `org.rust.STRUCT`.
    Struct,
    Enum,
    EnumVariant,
    Crate,
    Macro,
    /// `self` as a value, `org.rust.SELF_EXPRESSION` and `SELF_PARAMETER`.
    SelfValue,
    /// `?`, `org.rust.Q_OPERATOR`.
    QOperator,
}

impl Token {
    pub const ALL: [Token; 27] = [
        Token::Keyword,
        Token::String,
        Token::Escape,
        Token::Number,
        Token::Comment,
        Token::DocComment,
        Token::Function,
        Token::FunctionCall,
        Token::Type,
        Token::Constant,
        Token::Property,
        Token::Metadata,
        Token::Tag,
        Token::Attribute,
        Token::Heading,
        Token::Link,
        Token::Strong,
        Token::Emphasis,
        Token::Punctuation,
        Token::Operator,
        Token::Struct,
        Token::Enum,
        Token::EnumVariant,
        Token::Crate,
        Token::Macro,
        Token::SelfValue,
        Token::QOperator,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub token: Token,
}

/// The capture names in `queries/`. `plain` resets the parent, like `${}` in a string.
/// With several matches on the same node the last pattern wins, and captures on children sit
/// inside the parent.
const NAMES: [(&str, Option<Token>); 28] = [
    ("keyword", Some(Token::Keyword)),
    ("string", Some(Token::String)),
    ("escape", Some(Token::Escape)),
    ("number", Some(Token::Number)),
    ("comment", Some(Token::Comment)),
    ("doc_comment", Some(Token::DocComment)),
    ("function", Some(Token::Function)),
    ("function_call", Some(Token::FunctionCall)),
    ("type", Some(Token::Type)),
    ("constant", Some(Token::Constant)),
    ("property", Some(Token::Property)),
    ("metadata", Some(Token::Metadata)),
    ("tag", Some(Token::Tag)),
    ("attribute", Some(Token::Attribute)),
    ("heading", Some(Token::Heading)),
    ("link", Some(Token::Link)),
    ("strong", Some(Token::Strong)),
    ("emphasis", Some(Token::Emphasis)),
    ("punctuation", Some(Token::Punctuation)),
    ("operator", Some(Token::Operator)),
    ("struct", Some(Token::Struct)),
    ("enum", Some(Token::Enum)),
    ("enum_variant", Some(Token::EnumVariant)),
    ("crate", Some(Token::Crate)),
    ("macro", Some(Token::Macro)),
    ("self", Some(Token::SelfValue)),
    ("q_operator", Some(Token::QOperator)),
    ("plain", None),
];

fn config(language: Language) -> &'static HighlightConfiguration {
    static CONFIGS: [OnceLock<HighlightConfiguration>; LANGUAGES.len()] =
        [const { OnceLock::new() }; LANGUAGES.len()];
    CONFIGS[language as usize].get_or_init(|| build(language))
}

/// The inline grammar of Markdown, which the block grammar injects.
fn markdown_inline() -> &'static HighlightConfiguration {
    static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
    CONFIG.get_or_init(|| {
        new_config(
            tree_sitter_md::INLINE_LANGUAGE.into(),
            include_str!("../queries/markdown_inline.scm"),
            "",
        )
    })
}

fn build(language: Language) -> HighlightConfiguration {
    use Language::*;
    const JAVASCRIPT: &str = include_str!("../queries/javascript.scm");
    const TYPESCRIPT: &str = include_str!("../queries/typescript.scm");
    let (grammar, highlights, injections) = match language {
        Go => (
            tree_sitter_go::LANGUAGE,
            include_str!("../queries/go.scm").to_owned(),
            "",
        ),
        Java => (
            tree_sitter_java::LANGUAGE,
            include_str!("../queries/java.scm").to_owned(),
            "",
        ),
        JavaScript => (tree_sitter_javascript::LANGUAGE, JAVASCRIPT.to_owned(), ""),
        Json => (
            tree_sitter_json::LANGUAGE,
            include_str!("../queries/json.scm").to_owned(),
            "",
        ),
        Kotlin => (
            tree_sitter_kotlin_sg::LANGUAGE,
            include_str!("../queries/kotlin.scm").to_owned(),
            "",
        ),
        Markdown => (
            tree_sitter_md::LANGUAGE,
            include_str!("../queries/markdown.scm").to_owned(),
            include_str!("../queries/markdown_injections.scm"),
        ),
        Python => (
            tree_sitter_python::LANGUAGE,
            include_str!("../queries/python.scm").to_owned(),
            "",
        ),
        Rust => (
            tree_sitter_rust::LANGUAGE,
            include_str!("../queries/rust.scm").to_owned(),
            "",
        ),
        Toml => (
            tree_sitter_toml_ng::LANGUAGE,
            include_str!("../queries/toml.scm").to_owned(),
            "",
        ),
        Tsx => (
            tree_sitter_typescript::LANGUAGE_TSX,
            [JAVASCRIPT, TYPESCRIPT].concat(),
            "",
        ),
        // The JSX patterns in javascript.scm don't exist in the TypeScript grammar
        TypeScript => (
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            [strip_jsx(JAVASCRIPT), TYPESCRIPT].concat(),
            "",
        ),
        Xml => (
            tree_sitter_xml::LANGUAGE_XML,
            include_str!("../queries/xml.scm").to_owned(),
            "",
        ),
        Yaml => (
            tree_sitter_yaml::LANGUAGE,
            include_str!("../queries/yaml.scm").to_owned(),
            "",
        ),
    };
    new_config(grammar.into(), &highlights, injections)
}

fn strip_jsx(query: &str) -> &str {
    &query[..query.find("(jsx_").unwrap_or(query.len())]
}

fn new_config(
    language: tree_sitter::Language,
    highlights: &str,
    injections: &str,
) -> HighlightConfiguration {
    let mut config = HighlightConfiguration::new(language, "", highlights, injections, "")
        .unwrap_or_else(|e| panic!("invalid query: {e}"));
    config.configure(&NAMES.map(|(name, _)| name));
    config
}

fn injection(name: &str) -> Option<&'static HighlightConfiguration> {
    (name == "markdown_inline").then(markdown_inline)
}

/// Sorted and without overlap. Nested captures give the innermost token.
pub fn highlight(language: Language, text: &str) -> Vec<Span> {
    let config = config(language);
    let mut highlighter = Highlighter::new();
    let Ok(events) = highlighter.highlight(config, text.as_bytes(), None, None, |name| {
        injection(name).map(|injected| injected as &HighlightConfiguration)
    }) else {
        return Vec::new();
    };
    let mut stack: Vec<Option<Token>> = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    for event in events {
        match event {
            Ok(HighlightEvent::HighlightStart(capture)) => stack.push(NAMES[capture.0].1),
            Ok(HighlightEvent::HighlightEnd) => {
                stack.pop();
            }
            Ok(HighlightEvent::Source { start, end }) => {
                let Some(&Some(token)) = stack.last() else {
                    continue;
                };
                match spans.last_mut() {
                    Some(last) if last.token == token && last.range.end == start => {
                        last.range.end = end
                    }
                    _ => spans.push(Span {
                        range: start..end,
                        token,
                    }),
                }
            }
            // Cancelled or nested too deeply: show what we have
            Err(_) => break,
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(language: Language, text: &str) -> Vec<(&str, Token)> {
        highlight(language, text)
            .into_iter()
            .map(|span| (&text[span.range], span.token))
            .collect()
    }

    fn has(language: Language, text: &str, want: &[(&str, Token)]) {
        let got = tokens(language, text);
        for wanted in want {
            assert!(got.contains(wanted), "{wanted:?} missing in {got:?}");
        }
    }

    /// No token for these; they are shown as plain text.
    fn lacks(language: Language, text: &str, plain: &[&str]) {
        let got = tokens(language, text);
        for plain_text in plain {
            assert!(
                !got.iter().any(|(text, _)| text == plain_text),
                "{plain_text:?} has a token in {got:?}"
            );
        }
    }

    #[test]
    fn order_like_the_enum() {
        for (i, token) in Token::ALL.iter().enumerate() {
            assert_eq!(*token as usize, i, "{token:?}");
            assert!(
                NAMES.iter().any(|(_, named)| *named == Some(*token)),
                "{token:?} missing in NAMES"
            );
        }
        for (i, language) in LANGUAGES.iter().enumerate() {
            assert_eq!(*language as usize, i, "{language:?}");
        }
    }

    #[test]
    fn all_queries_build() {
        for language in LANGUAGES {
            config(language);
        }
        markdown_inline();
    }

    #[test]
    fn file_extension() {
        assert_eq!(Language::from_path("a/b.kt"), Some(Language::Kotlin));
        assert_eq!(Language::from_path("App.TSX"), Some(Language::Tsx));
        assert_eq!(Language::from_path("Makefile"), None);
        assert_eq!(Language::from_path("a.txt"), None);
    }

    #[test]
    fn rust() {
        use Token::*;
        has(
            Language::Rust,
            "/// Doc\nfn main() { let s = \"a\\n\"; 42 }",
            &[
                ("/// Doc\n", DocComment),
                ("fn", Keyword),
                ("main", Function),
                ("let", Keyword),
                ("\\n", Escape),
                ("42", Number),
            ],
        );
        let text = "fn f() { let a = UiTheme::dark(); std::env::args(); Some(A::B(MAX)); }";
        has(
            Language::Rust,
            text,
            &[
                ("UiTheme", Struct),
                ("::", Operator),
                ("{", Punctuation),
                ("Some", EnumVariant),
                ("B", EnumVariant),
                ("MAX", Constant),
                ("std", Crate),
            ],
        );
        lacks(Language::Rust, text, &["env"]);
    }

    #[test]
    fn rust_like_rustrover() {
        use Token::*;
        let text = "use std::path::{Path, PathBuf};\nuse jetdiff_files::FileStatus;\n\
            fn f(dir: &Path) -> Result<Self, String> {\n\
            let a = thread::scope(|s| s.spawn(|| verify(dir)));\n\
            let b = a.join().unwrap()?;\n\
            match b { FileStatus::Added => println!(), Token::Foo { .. } => {} }\n\
            self.root\n}\n";
        has(
            Language::Rust,
            text,
            &[
                ("std", Crate),
                ("jetdiff_files", Crate),
                ("PathBuf", Struct),
                ("FileStatus", Struct),
                ("Result", Enum),
                ("Self", Keyword),
                ("String", Struct),
                ("scope", Function),
                ("spawn", Function),
                ("verify", Function),
                ("unwrap", Function),
                ("?", QOperator),
                ("Added", EnumVariant),
                ("Foo", EnumVariant),
                ("println!", Macro),
                ("self", SelfValue),
            ],
        );
        lacks(Language::Rust, text, &["path", "thread", "dir", "a"]);
    }

    #[test]
    fn rust_attribute_in_one_color() {
        let got = tokens(
            Language::Rust,
            "#[derive(Debug, serde::Serialize)]\n#[cfg(all(unix, not(test)))]\n",
        );
        assert_eq!(
            got,
            [
                ("#[derive(Debug, serde::Serialize)]", Token::Metadata),
                ("#[cfg(all(unix, not(test)))]", Token::Metadata)
            ]
        );
    }

    #[test]
    fn operators_and_punctuation() {
        use Token::*;
        has(
            Language::Go,
            "package main\nfunc f() { a &^= b }\n",
            &[("&^=", Operator), ("{", Punctuation)],
        );
        has(
            Language::Java,
            "class A { int f() { return a ? b : c; } }",
            &[("?", Operator), (":", Operator), (";", Punctuation)],
        );
        has(
            Language::Kotlin,
            "val a = b?.c ?: d",
            &[("?.", Operator), ("?:", Operator)],
        );
        has(
            Language::JavaScript,
            "a ??= b < c ? d : e;",
            &[("??=", Operator), ("<", Operator), (":", Operator)],
        );
        has(
            Language::JavaScript,
            "const a = <div>{b}</div>;",
            &[("<", Punctuation), ("div", Tag)],
        );
    }

    #[test]
    fn kotlin() {
        use Token::*;
        let text = "/** Doc */\n@Test\nfun f(x: Int): String = \"hi $x\"\nval MAX = y.z\n";
        has(
            Language::Kotlin,
            text,
            &[
                ("/** Doc */", DocComment),
                ("@Test", Metadata),
                ("fun", Keyword),
                ("f", Function),
                ("\"hi $", String),
                ("MAX", Constant),
                ("z", Property),
                ("Int", Type),
            ],
        );
        lacks(Language::Kotlin, text, &["x"]);
        has(
            Language::Kotlin,
            "data class A(val b: C?)",
            &[("A", Type), ("C", Type)],
        );
        let text = "fun f() { return visible }";
        has(Language::Kotlin, text, &[("return", Keyword)]);
        lacks(Language::Kotlin, text, &["visible"]);
    }

    #[test]
    fn python() {
        use Token::*;
        let text = "\"\"\"Doc\"\"\"\n@dataclass\n@app.route(\"/\")\nclass A(B):\n    \"\"\"Doc A\"\"\"\n    \
            def f(self, x: int) -> str:\n        if x is not None:\n            \"s\"\n        \
            return f\"{x}\\n\" + g(self.h()) # c\n";
        has(
            Language::Python,
            text,
            &[
                ("\"\"\"Doc\"\"\"", DocComment),
                ("\"\"\"Doc A\"\"\"", DocComment),
                ("@dataclass", Metadata),
                ("@app.route", Metadata),
                ("class", Keyword),
                ("A", Type),
                ("def", Keyword),
                ("f", Function),
                ("self", SelfValue),
                ("->", Operator),
                ("is", Keyword),
                ("not", Keyword),
                ("None", Keyword),
                ("\"s\"", String),
                ("\\n", Escape),
                ("g", FunctionCall),
                ("h", FunctionCall),
                ("# c", Comment),
            ],
        );
        lacks(Language::Python, text, &["x", "int", "B", "{x}"]);
        assert_eq!(Language::from_path("a/b.py"), Some(Language::Python));
        assert_eq!(Language::from_path("gui.pyw"), Some(Language::Python));
    }

    #[test]
    fn python_decorator_is_only_the_name() {
        let got = tokens(
            Language::Python,
            "@a.b.c\n@lru_cache(maxsize=SIZE)\n@x.y.z(1)\ndef f(): pass\n",
        );
        for name in ["@a.b.c", "@lru_cache", "@x.y.z"] {
            assert!(got.contains(&(name, Token::Metadata)), "{name} in {got:?}");
        }
        lacks(
            Language::Python,
            "@lru_cache(maxsize=SIZE)\ndef f(): pass\n",
            &["maxsize", "SIZE"],
        );
    }

    #[test]
    fn python_docstring_after_shebang() {
        use Token::*;
        has(
            Language::Python,
            "#!/usr/bin/env python3\n\"\"\"Doc.\"\"\"\ndef k():\n    \"a\" \"b\"\n",
            &[
                ("\"\"\"Doc.\"\"\"", DocComment),
                ("\"a\" \"b\"", DocComment),
            ],
        );
    }

    #[test]
    fn python_fstring() {
        use Token::*;
        let text = "s = f\"{{lit}} {x!r:>10} {y:{w}}\"\nreturn ...\n";
        has(
            Language::Python,
            text,
            &[("{{", Escape), ("}}", Escape), ("...", Keyword)],
        );
        let got = tokens(Language::Python, text);
        assert!(
            got.iter()
                .any(|&(text, token)| text.contains("!r") && token == String),
            "{got:?}"
        );
        assert!(
            got.iter()
                .any(|&(text, token)| text.contains(">10") && token == String),
            "{got:?}"
        );
        lacks(Language::Python, text, &["x", "w"]);
    }

    #[test]
    fn java() {
        use Token::*;
        let text = "/** Doc */\n@Override\npublic int f(String s) { return MAX + this.n + 1; }\n";
        has(
            Language::Java,
            text,
            &[
                ("/** Doc */", DocComment),
                ("@Override", Metadata),
                ("public", Keyword),
                ("int", Keyword),
                ("f", Function),
                ("MAX", Constant),
                ("this", Keyword),
                ("n", Property),
                ("1", Number),
                ("String", Type),
            ],
        );
        lacks(Language::Java, text, &["s"]);
        has(Language::Java, "class URL {}", &[("URL", Type)]);
    }

    #[test]
    fn xml() {
        use Token::*;
        has(
            Language::Xml,
            "<?xml version=\"1.0\"?>\n<!-- k -->\n<a href=\"x\">&amp;<b/></a>\n",
            &[
                ("<!-- k -->", Comment),
                ("a", Tag),
                ("href", Attribute),
                ("\"x\"", String),
                ("&amp;", Escape),
                ("b", Tag),
            ],
        );
    }

    #[test]
    fn calls() {
        use Token::*;
        has(
            Language::Go,
            "package main\nfunc f() { g(); a.h() }\n",
            &[("f", Function), ("g", FunctionCall), ("h", FunctionCall)],
        );
        has(
            Language::TypeScript,
            "function f() { g(); a.b(); }",
            &[("f", Function), ("g", FunctionCall), ("b", FunctionCall)],
        );
        has(
            Language::Kotlin,
            "val a = g(A(), b.c())",
            &[("g", FunctionCall), ("A", Type), ("c", FunctionCall)],
        );
        has(
            Language::Java,
            "class A { void f() { g(); b.c(); } }",
            &[("g", FunctionCall), ("c", FunctionCall)],
        );
        has(
            Language::Rust,
            "fn f() { UiTheme::dark(); g(b.c()); std::env::args(); }",
            // Rust colors calls like declarations
            &[
                ("dark", Function),
                ("g", Function),
                ("c", Function),
                ("args", Function),
            ],
        );
        has(
            Language::JavaScript,
            "class A {}\nnew B();",
            &[("A", Type), ("B", Type)],
        );
    }

    #[test]
    fn tsx_and_jsx() {
        use Token::*;
        has(
            Language::Tsx,
            "const a: number = <div className=\"x\">{b}</div>;",
            &[
                ("const", Keyword),
                ("number", Keyword),
                ("div", Tag),
                ("className", Attribute),
            ],
        );
    }

    #[test]
    fn keys() {
        has(
            Language::Yaml,
            "name: value\n",
            &[("name", Token::Keyword), ("value", Token::String)],
        );
        has(
            Language::Toml,
            "[t]\nname = 1\n",
            &[("t", Token::Keyword), ("name", Token::Keyword)],
        );
        has(
            Language::Json,
            "{\"name\": \"v\"}",
            &[("\"name\"", Token::Property), ("\"v\"", Token::String)],
        );
    }

    #[test]
    fn markdown() {
        use Token::*;
        let got = tokens(
            Language::Markdown,
            "# Title\n\nSome **bold** text.\n\n```rust\nfn f() {}\n```\n",
        );
        assert!(
            got.iter()
                .any(|&(text, token)| text.contains("Title") && token == Heading),
            "{got:?}"
        );
        assert!(got.contains(&("**bold**", Strong)), "{got:?}");
        assert!(!got.iter().any(|&(text, _)| text.contains("fn")), "{got:?}");
    }

    #[test]
    fn template_string() {
        let got = tokens(Language::JavaScript, "`a${b}c`");
        assert!(
            !got.iter()
                .any(|&(text, token)| text.contains('b') && token == Token::String),
            "{got:?}"
        );
    }
}
