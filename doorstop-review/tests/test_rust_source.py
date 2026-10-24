from doorstop_review.rust_source import rust_fn

SOURCE = """\
fn login(password: &str) -> bool {
    password == "secret }"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wrong password is rejected.
    #[test]
    fn rejects() {
        let brace = '{';
        // a } in a comment
        assert!(!login(r#"wr}ng"#));
    }
}
"""


def write(tmp_path, source=SOURCE):
    path = tmp_path / "login.rs"
    path.write_text(source)
    return path


def test_finds_a_top_level_function_despite_braces_in_strings(tmp_path):
    assert rust_fn(write(tmp_path), "login") == (
        'fn login(password: &str) -> bool {\n    password == "secret }"\n}'
    )


def test_finds_a_function_in_a_module_with_its_attributes_and_doc_comments(tmp_path):
    found = rust_fn(write(tmp_path), "tests::rejects")
    assert found.startswith("/// A wrong password is rejected.\n#[test]\nfn rejects() {")
    assert found.endswith('assert!(!login(r#"wr}ng"#));\n}')


def test_the_module_path_must_match(tmp_path):
    assert rust_fn(write(tmp_path), "rejects") is None
    assert rust_fn(write(tmp_path), "other::rejects") is None


def test_a_missing_file_or_function_is_none(tmp_path):
    assert rust_fn(tmp_path / "missing.rs", "login") is None
    assert rust_fn(write(tmp_path), "missing") is None
