use echo_capnp::echo::{EchoParams, EchoResults, Server};

capnp::generated_code!(pub mod echo_capnp);

/// Echo service used to test good communication between client and server.
#[derive(Default)]
pub struct EchoServer;

impl Server for EchoServer {
    async fn echo(
        self: capnp::capability::Rc<Self>,
        params: EchoParams,
        mut results: EchoResults,
    ) -> Result<(), capnp::Error> {
        let message = params.get()?.get_message()?.to_str()?;
        results.get().set_reply(message);
        Ok(())
    }
}
