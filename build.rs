fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/teleop.capnp")
        .default_parent_module(vec!["operate".to_owned(), "capnp".to_owned()])
        .run()
        .expect("compiled teleop");

    capnpc::CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/echo.capnp")
        .default_parent_module(vec!["operate".to_owned(), "capnp::echo".to_owned()])
        .run()
        .expect("compiled echo");
}
