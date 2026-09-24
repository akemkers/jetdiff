//! Syntax highlighting med tree-sitter. Gir tokens med byte-intervaller for en hel fil, så
//! slettede linjer får riktig syntaks. Tokenene tilsvarer IntelliJ sine `DEFAULT_*`-nøkler.

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
    Rust,
    Toml,
    Tsx,
    TypeScript,
    Xml,
    Yaml,
}

/// Samme rekkefølge som enumen, så `lang as usize` er indeksen.
const LANGUAGES: [Language; 12] = [
    Language::Go,
    Language::Java,
    Language::JavaScript,
    Language::Json,
    Language::Kotlin,
    Language::Markdown,
    Language::Rust,
    Language::Toml,
    Language::Tsx,
    Language::TypeScript,
    Language::Xml,
    Language::Yaml,
];

impl Language {
    pub fn from_path(path: &str) -> Option<Self> {
        let ext = path.rsplit_once('.')?.1;
        Some(match ext.to_ascii_lowercase().as_str() {
            "go" => Self::Go,
            "java" => Self::Java,
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "json" => Self::Json,
            "kt" | "kts" => Self::Kotlin,
            "md" | "markdown" => Self::Markdown,
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
    Constant,
    Property,
    Metadata,
    Tag,
    Attribute,
    Heading,
    Link,
    Strong,
    Emphasis,
}

impl Token {
    pub const ALL: [Token; 16] = [
        Token::Keyword,
        Token::String,
        Token::Escape,
        Token::Number,
        Token::Comment,
        Token::DocComment,
        Token::Function,
        Token::Constant,
        Token::Property,
        Token::Metadata,
        Token::Tag,
        Token::Attribute,
        Token::Heading,
        Token::Link,
        Token::Strong,
        Token::Emphasis,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub token: Token,
}

/// Capture-navnene i `queries/`. `plain` nullstiller foreldren, som `${}` i en streng.
const NAMES: [(&str, Option<Token>); 17] = [
    ("keyword", Some(Token::Keyword)),
    ("string", Some(Token::String)),
    ("escape", Some(Token::Escape)),
    ("number", Some(Token::Number)),
    ("comment", Some(Token::Comment)),
    ("doc_comment", Some(Token::DocComment)),
    ("function", Some(Token::Function)),
    ("constant", Some(Token::Constant)),
    ("property", Some(Token::Property)),
    ("metadata", Some(Token::Metadata)),
    ("tag", Some(Token::Tag)),
    ("attribute", Some(Token::Attribute)),
    ("heading", Some(Token::Heading)),
    ("link", Some(Token::Link)),
    ("strong", Some(Token::Strong)),
    ("emphasis", Some(Token::Emphasis)),
    ("plain", None),
];

fn config(lang: Language) -> &'static HighlightConfiguration {
    static CONFIGS: [OnceLock<HighlightConfiguration>; LANGUAGES.len()] =
        [const { OnceLock::new() }; LANGUAGES.len()];
    CONFIGS[lang as usize].get_or_init(|| build(lang))
}

/// Inline-grammatikken til Markdown, som blokk-grammatikken injiserer.
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

fn build(lang: Language) -> HighlightConfiguration {
    use Language::*;
    const JS: &str = include_str!("../queries/javascript.scm");
    const TS: &str = include_str!("../queries/typescript.scm");
    let (language, highlights, injections) = match lang {
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
        JavaScript => (tree_sitter_javascript::LANGUAGE, JS.to_owned(), ""),
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
        Tsx => (tree_sitter_typescript::LANGUAGE_TSX, [JS, TS].concat(), ""),
        // JSX-mønstrene i javascript.scm finnes ikke i TypeScript-grammatikken
        TypeScript => (
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
            [strip_jsx(JS), TS].concat(),
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
    new_config(language.into(), &highlights, injections)
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
        .unwrap_or_else(|e| panic!("ugyldig query: {e}"));
    config.configure(&NAMES.map(|(n, _)| n));
    config
}

fn injection(name: &str) -> Option<&'static HighlightConfiguration> {
    (name == "markdown_inline").then(markdown_inline)
}

/// Sortert og uten overlapp. Nestede captures gir det innerste tokenet.
pub fn highlight(lang: Language, text: &str) -> Vec<Span> {
    let config = config(lang);
    let mut highlighter = Highlighter::new();
    let Ok(events) = highlighter.highlight(config, text.as_bytes(), None, None, |name| {
        injection(name).map(|c| c as &HighlightConfiguration)
    }) else {
        return Vec::new();
    };
    let mut stack: Vec<Option<Token>> = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    for event in events {
        match event {
            Ok(HighlightEvent::HighlightStart(h)) => stack.push(NAMES[h.0].1),
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
            // Avbrutt eller for dypt nestet: vis det vi har
            Err(_) => break,
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(lang: Language, text: &str) -> Vec<(&str, Token)> {
        highlight(lang, text)
            .into_iter()
            .map(|s| (&text[s.range], s.token))
            .collect()
    }

    fn has(lang: Language, text: &str, want: &[(&str, Token)]) {
        let got = tokens(lang, text);
        for w in want {
            assert!(got.contains(w), "{w:?} mangler i {got:?}");
        }
    }

    #[test]
    fn alle_queries_bygger() {
        for lang in LANGUAGES {
            config(lang);
        }
        markdown_inline();
    }

    #[test]
    fn filendelse() {
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
    }

    #[test]
    fn kotlin() {
        use Token::*;
        let text = "/** Doc */\n@Test\nfun f(x: Int): String = \"hei $x\"\nval MAX = y.z\n";
        has(
            Language::Kotlin,
            text,
            &[
                ("/** Doc */", DocComment),
                ("@Test", Metadata),
                ("fun", Keyword),
                ("f", Function),
                ("\"hei $", String),
                ("MAX", Constant),
                ("z", Property),
            ],
        );
        let got = tokens(Language::Kotlin, text);
        assert!(!got.iter().any(|&(t, _)| t == "Int" || t == "x"), "{got:?}");
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
            ],
        );
        let got = tokens(Language::Java, text);
        assert!(
            !got.iter().any(|&(t, _)| t == "String" || t == "s"),
            "{got:?}"
        );
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
    fn kall_er_vanlig_tekst() {
        // IntelliJ farger bare deklarasjonen
        let got = tokens(Language::Go, "package main\nfunc f() { g(); a.h() }\n");
        assert!(got.contains(&("f", Token::Function)), "{got:?}");
        assert!(!got.iter().any(|&(t, _)| t == "g" || t == "h"), "{got:?}");
        let got = tokens(Language::TypeScript, "function f() { a.b(); }");
        assert!(got.contains(&("f", Token::Function)), "{got:?}");
        assert!(!got.iter().any(|&(t, _)| t == "b"), "{got:?}");
    }

    #[test]
    fn tsx_og_jsx() {
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
    fn nokler() {
        has(
            Language::Yaml,
            "navn: verdi\n",
            &[("navn", Token::Keyword), ("verdi", Token::String)],
        );
        has(
            Language::Toml,
            "[t]\nnavn = 1\n",
            &[("t", Token::Keyword), ("navn", Token::Keyword)],
        );
        has(
            Language::Json,
            "{\"navn\": \"v\"}",
            &[("\"navn\"", Token::Property), ("\"v\"", Token::String)],
        );
    }

    #[test]
    fn markdown() {
        use Token::*;
        let got = tokens(
            Language::Markdown,
            "# Tittel\n\nLitt **fet** tekst.\n\n```rust\nfn f() {}\n```\n",
        );
        assert!(
            got.iter()
                .any(|&(t, k)| t.contains("Tittel") && k == Heading),
            "{got:?}"
        );
        assert!(got.contains(&("**fet**", Strong)), "{got:?}");
        assert!(!got.iter().any(|&(t, _)| t.contains("fn")), "{got:?}");
    }

    #[test]
    fn template_string() {
        let got = tokens(Language::JavaScript, "`a${b}c`");
        assert!(
            !got.iter()
                .any(|&(t, k)| t.contains('b') && k == Token::String),
            "{got:?}"
        );
    }
}
