use super::*;

#[test]
fn guard_detects_actual_syntax_without_matching_comments_or_strings() {
    let source = "fn sample() { let mut counter = 0; counter += 1; let text = \"x += 1\"; }";
    let mut checker = Checker::default();
    checker.visit_file(&syn::parse_file(source).unwrap());
    assert_eq!(checker.problems.len(), 1);
    let mut checker = Checker::default();
    checker.visit_file(&syn::parse_file("fn sample() { let text = \"x += 1\"; }").unwrap());
    assert!(checker.problems.is_empty());
}

#[test]
fn guard_checks_function_and_struct_limits() {
    let fields = (0..31)
        .map(|n| format!("field_{n}: u8,"))
        .collect::<String>();
    let statements = "let value = 1;\n".repeat(81);
    let source = format!("struct Large {{ {fields} }}\nfn long() {{\n{statements}}}");
    let mut checker = Checker::default();
    checker.visit_file(&syn::parse_file(&source).unwrap());
    assert_eq!(checker.problems.len(), 2);
}
