#[cfg(all(test, feature = "ci"))]
mod tests {
    #[test]
    fn compile_pass_tests() {
        let t = trybuild::TestCases::new();
        t.pass("tests/passing/*.rs");
    }

    #[test]
    fn compile_fail_tests() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/failing/*.rs");
    }
}
