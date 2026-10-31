#[test]
#[cfg_attr(miri, ignore = "trybuild runs cargo, which Miri can't")]
fn compile_fail() {
    trybuild::TestCases::new().compile_fail("tests/compile_fail/*.rs");
}
