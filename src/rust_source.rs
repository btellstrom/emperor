//! Finds functions in Rust source and fingerprints them by their tokens.
//!
//! Tokens leave out whitespace and comments, so reformatting or commenting a function
//! keeps its fingerprint, while any change to its code or attributes does not.

use std::fmt::Write;

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Item, ItemFn};

use crate::fingerprint::sha256;
use crate::rust_tags::JUSTIFIES;

/// Attributes that describe a function rather than change it.
const IGNORED_ATTRIBUTES: [&str; 2] = ["doc", JUSTIFIES];

/// The fingerprint of the function at `path` (e.g. `tests::hashes_password`),
/// or `None` when the source has no such function.
pub fn fingerprint_function(source: &str, path: &str) -> syn::Result<Option<String>> {
    let file = syn::parse_file(source)?;
    Ok(find_function(&file.items, path)
        .map(|function| sha256(canonical_text(function.to_token_stream()).as_bytes())))
}

/// The line the function at `path` starts on, counting its attributes.
pub fn function_line(source: &str, path: &str) -> syn::Result<Option<usize>> {
    let file = syn::parse_file(source)?;
    Ok(find_function(&file.items, path).map(|function| function.span().start().line))
}

fn find_function<'a>(items: &'a [Item], path: &str) -> Option<&'a ItemFn> {
    match path.split_once("::") {
        Some((module, rest)) => items.iter().find_map(|item| match item {
            Item::Mod(inline) if inline.ident == module => inline
                .content
                .as_ref()
                .and_then(|(_, items)| find_function(items, rest)),
            _ => None,
        }),
        None => items.iter().find_map(|item| match item {
            Item::Fn(function) if function.sig.ident == path => Some(function),
            _ => None,
        }),
    }
}

/// The tokens as text with fixed spacing, without ignored attributes.
///
/// Written out by hand so the fingerprint does not depend on how a library version
/// happens to print tokens.
fn canonical_text(tokens: TokenStream) -> String {
    let mut text = String::new();
    write_canonical(tokens, &mut text);
    text
}

fn write_canonical(tokens: TokenStream, text: &mut String) {
    let mut tokens = tokens.into_iter().peekable();
    while let Some(token) = tokens.next() {
        if starts_ignored_attribute(&token, tokens.peek()) {
            tokens.next();
            continue;
        }
        match token {
            TokenTree::Group(group) => {
                let (open, close) = delimiters(group.delimiter());
                text.push_str(open);
                write_canonical(group.stream(), text);
                text.push_str(close);
            }
            TokenTree::Punct(punct) => {
                text.push(punct.as_char());
                if punct.spacing() == Spacing::Alone {
                    text.push(' ');
                }
            }
            TokenTree::Ident(ident) => write!(text, "{ident} ").expect("writing to a String"),
            TokenTree::Literal(literal) => write!(text, "{literal} ").expect("writing to a String"),
        }
    }
}

/// Whether `token` and `next` are the `#` and `[...]` of an ignored outer attribute.
fn starts_ignored_attribute(token: &TokenTree, next: Option<&TokenTree>) -> bool {
    let (TokenTree::Punct(pound), Some(TokenTree::Group(body))) = (token, next) else {
        return false;
    };
    pound.as_char() == '#'
        && body.delimiter() == Delimiter::Bracket
        && attribute_name(body.stream())
            .is_some_and(|name| IGNORED_ATTRIBUTES.contains(&name.as_str()))
}

/// The last segment of an attribute's path, e.g. `justifies` for `emperor_tags::justifies(...)`.
fn attribute_name(body: TokenStream) -> Option<String> {
    let mut name = None;
    for token in body {
        match token {
            TokenTree::Ident(ident) => name = Some(ident.to_string()),
            TokenTree::Punct(punct) if punct.as_char() == ':' => {}
            _ => break,
        }
    }
    name
}

fn delimiters(delimiter: Delimiter) -> (&'static str, &'static str) {
    match delimiter {
        Delimiter::Parenthesis => ("(", ")"),
        Delimiter::Brace => ("{", "}"),
        Delimiter::Bracket => ("[", "]"),
        Delimiter::None => ("", ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
#[test]
fn rejects_bad_password() {
    assert!(!login("alice", "wrong"));
}

fn other() {}

mod tests {
    mod deeper {
        fn nested() {}
    }
}
"#;

    fn fingerprint(source: &str, path: &str) -> Option<String> {
        fingerprint_function(source, path).unwrap()
    }

    fn original() -> String {
        fingerprint(SOURCE, "rejects_bad_password").unwrap()
    }

    #[test]
    fn reports_the_line_a_function_starts_on() {
        assert_eq!(
            function_line(SOURCE, "rejects_bad_password").unwrap(),
            Some(2)
        );
        assert_eq!(function_line(SOURCE, "other").unwrap(), Some(7));
    }

    #[test]
    fn the_justifies_attribute_does_not_change_the_fingerprint() {
        let tagged = SOURCE.replace("#[test]", "#[test]\n#[justifies(\"REQ001\", \"REQ002\")]");
        assert_eq!(
            fingerprint(&tagged, "rejects_bad_password"),
            Some(original())
        );
        let qualified =
            SOURCE.replace("#[test]", "#[emperor_tags::justifies(\"REQ001\")]\n#[test]");
        assert_eq!(
            fingerprint(&qualified, "rejects_bad_password"),
            Some(original())
        );
    }

    #[test]
    fn finds_a_top_level_function() {
        assert!(fingerprint(SOURCE, "rejects_bad_password").is_some());
    }

    #[test]
    fn finds_a_function_by_its_module_path() {
        assert!(fingerprint(SOURCE, "tests::deeper::nested").is_some());
    }

    #[test]
    fn a_missing_function_has_no_fingerprint() {
        assert_eq!(fingerprint(SOURCE, "missing"), None);
        assert_eq!(fingerprint(SOURCE, "tests::other"), None);
    }

    #[test]
    fn invalid_source_is_an_error() {
        assert!(fingerprint_function("fn broken( {", "broken").is_err());
    }

    #[test]
    fn formatting_does_not_change_the_fingerprint() {
        let reformatted = SOURCE
            .replace("    assert!", "\n\n        assert!  ")
            .replace(", ", " ,");
        assert_eq!(
            fingerprint(&reformatted, "rejects_bad_password"),
            Some(original())
        );
    }

    #[test]
    fn comments_do_not_change_the_fingerprint() {
        let commented = SOURCE
            .replace("#[test]", "/// Documented.\n#[test]")
            .replace("    assert!", "    // a comment\n    /* another */ assert!");
        assert_eq!(
            fingerprint(&commented, "rejects_bad_password"),
            Some(original())
        );
    }

    #[test]
    fn changing_the_body_changes_the_fingerprint() {
        let changed = SOURCE.replace("wrong", "Wrong");
        assert_ne!(
            fingerprint(&changed, "rejects_bad_password"),
            Some(original())
        );
    }

    #[test]
    fn changing_an_attribute_changes_the_fingerprint() {
        let ignored = SOURCE.replace("#[test]", "#[test]\n#[ignore]");
        assert_ne!(
            fingerprint(&ignored, "rejects_bad_password"),
            Some(original())
        );
    }

    #[test]
    fn changing_another_function_does_not_change_the_fingerprint() {
        let changed = SOURCE.replace("fn other() {}", "fn other() { panic!() }");
        assert_eq!(
            fingerprint(&changed, "rejects_bad_password"),
            Some(original())
        );
    }
}
