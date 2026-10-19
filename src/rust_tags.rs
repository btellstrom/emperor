//! Finds the `#[justifies(...)]` attributes in Rust source.

use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{Attribute, Item, LitStr, Token};

/// The attribute that tags a function as a justification.
pub const JUSTIFIES: &str = "justifies";

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
