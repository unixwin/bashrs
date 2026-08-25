use rubash::lexer::tokenize;
use rubash::parser::parse;

#[test]
fn nested_then_sequential_heredocs_attach_bodies_to_their_commands() {
    let ast = parse(&tokenize(
        "if true; then cat <<A; fi\none\nA\ncat <<B\ntwo\nB\n",
    ));

    let outer = &ast.commands[0];
    let if_command = outer.if_command.as_ref().expect("if command");
    assert_eq!(if_command.then_body[0].heredoc_redirects[0].body.as_deref(), Some("one\n"));
    assert_eq!(ast.commands[1].heredoc_redirects[0].body.as_deref(), Some("two\n"));
}
