//! Finds functions in Rust source and fingerprints them by their tokens.
//!
//! Tokens leave out whitespace and comments, so reformatting or commenting a function
//! keeps its fingerprint, while any change to its code or attributes does not.

use std::fmt::Write;

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, Item, ItemFn, LitStr, Token};

use crate::fingerprint::sha256;

/// The attribute that tags a function as a justification.
pub const JUSTIFIES: &str = "justifies";

/// Attributes that describe a function rather than change it.
const IGNORED_ATTRIBUTES: [&str; 2] = ["doc", JUSTIFIES];

/// The fingerprint of the function at `path` (e.g. `tests::hashes_password`),
/// or `None` when the source has no such function.
pub fn fingerprint_function(source: &str, path: &str) -> syn::Result<Option<String>> {
    let file = syn::parse_file(source)?;
    Ok(find_function(&file.items, path)
        .map(|function| sha256(canonical_text(function.to_token_stream()).as_bytes())))
}

/// A `#[justifies(...)]` attribute found in Rust source.
#[derive(Debug, PartialEq, Eq)]
pub struct Tag {
    /// The line the attribute is on.
    pub line: usize,
    pub kind: TagKind,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TagKind {
    /// On a function emperor can address by its module path.
    Justifies {
        function: String,
        requirements: Vec<String>,
    },
    /// On something other than a module-level function.
    Misplaced,
    /// The attribute's arguments are not requirement IDs as string literals.
    InvalidArguments,
}

/// Every `justifies` attribute in the source, in line order.
pub fn justifies_tags(source: &str) -> syn::Result<Vec<Tag>> {
    let file = syn::parse_file(source)?;
    let mut tags = Vec::new();
    collect_function_tags(&file.items, "", &mut tags);

    // Any other `justifies` attribute is somewhere emperor cannot address.
    let mut every_attribute = JustifiesLines::default();
    every_attribute.visit_file(&file);
    for line in every_attribute.0 {
        if !tags.iter().any(|tag| tag.line == line) {
            tags.push(Tag {
                line,
                kind: TagKind::Misplaced,
            });
        }
    }
    tags.sort_by_key(|tag| tag.line);
    Ok(tags)
}

/// Tags on functions at module level, which emperor addresses by module path.
fn collect_function_tags(items: &[Item], module: &str, tags: &mut Vec<Tag>) {
    for item in items {
        match item {
            Item::Fn(function) => {
                let path = format!("{module}{}", function.sig.ident);
                let attributes = function
                    .attrs
                    .iter()
                    .filter(|attribute| is_justifies(attribute));
                tags.extend(attributes.map(|attribute| function_tag(attribute, &path)));
            }
            Item::Mod(inline) => {
                if let Some((_, items)) = &inline.content {
                    collect_function_tags(items, &format!("{module}{}::", inline.ident), tags);
                }
            }
            _ => {}
        }
    }
}

fn function_tag(attribute: &Attribute, function: &str) -> Tag {
    let requirements = attribute
        .parse_args_with(Punctuated::<LitStr, Token![,]>::parse_terminated)
        .map(|ids| ids.iter().map(LitStr::value).collect());
    Tag {
        line: line_of(attribute),
        kind: match requirements {
            Ok(requirements) => TagKind::Justifies {
                function: function.to_string(),
                requirements,
            },
            Err(_) => TagKind::InvalidArguments,
        },
    }
}

fn is_justifies(attribute: &Attribute) -> bool {
    attribute
        .path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == JUSTIFIES)
}

fn line_of(attribute: &Attribute) -> usize {
    attribute.pound_token.span.start().line
}

/// The lines of every `justifies` attribute, wherever it is.
#[derive(Default)]
struct JustifiesLines(Vec<usize>);

impl<'ast> Visit<'ast> for JustifiesLines {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        if is_justifies(attribute) {
            self.0.push(line_of(attribute));
        }
    }
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

    fn tags(source: &str) -> Vec<Tag> {
        justifies_tags(source).unwrap()
    }

    fn justifies(line: usize, function: &str, requirements: &[&str]) -> Tag {
        Tag {
            line,
            kind: TagKind::Justifies {
                function: function.to_string(),
                requirements: requirements.iter().map(|uid| uid.to_string()).collect(),
            },
        }
    }

    #[test]
    fn finds_a_tagged_function_and_its_requirements() {
        let source =
            "#[test]\n#[justifies(\"REQ001\", \"REQ002\")]\nfn login() {}\nfn other() {}\n";
        assert_eq!(tags(source), [justifies(2, "login", &["REQ001", "REQ002"])]);
    }

    #[test]
    fn recognises_the_fully_qualified_attribute() {
        let source = "#[emperor_tags::justifies(\"REQ001\")]\nfn login() {}\n";
        assert_eq!(tags(source), [justifies(1, "login", &["REQ001"])]);
    }

    #[test]
    fn addresses_tagged_functions_in_modules_by_module_path() {
        let source = "mod tests {\n    mod inner {\n        #[justifies(\"REQ001\")]\n        fn login() {}\n    }\n}\n";
        assert_eq!(
            tags(source),
            [justifies(3, "tests::inner::login", &["REQ001"])]
        );
    }

    #[test]
    fn the_attribute_on_anything_but_a_module_level_function_is_misplaced() {
        let source = "#[justifies(\"REQ001\")]\nstruct Policy;\n\
                      impl Policy {\n    #[justifies(\"REQ001\")]\n    fn method() {}\n}\n\
                      fn outer() {\n    #[justifies(\"REQ001\")]\n    fn inner() {}\n}\n";
        let misplaced = |line| Tag {
            line,
            kind: TagKind::Misplaced,
        };
        assert_eq!(tags(source), [misplaced(1), misplaced(4), misplaced(8)]);
    }

    #[test]
    fn arguments_that_are_not_string_literals_are_invalid() {
        let source = "#[justifies(REQ001)]\nfn login() {}\n";
        assert_eq!(
            tags(source),
            [Tag {
                line: 1,
                kind: TagKind::InvalidArguments
            }]
        );
    }

    #[test]
    fn other_attributes_are_not_tags() {
        assert_eq!(tags("#[test]\n#[cfg(test)]\nfn login() {}\n"), []);
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
