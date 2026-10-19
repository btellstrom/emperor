use emperor_tags::justifies;

#[justifies("SPEC024")]
fn answer() -> u32 {
    42
}

#[test]
fn the_attribute_leaves_the_function_unchanged() {
    assert_eq!(answer(), 42);
}

#[justifies("SPEC024")]
#[test]
fn the_attribute_works_alongside_test() {}
