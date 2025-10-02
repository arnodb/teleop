use capnp::capability::Promise;
use capnp_rpc::pry;
use echo_capnp::echo::{EchoParams, EchoResults, Server};

capnp::generated_code!(pub mod echo_capnp);

#[derive(Default)]
pub struct EchoServer;

impl Server for EchoServer {
    fn echo(
        &mut self,
        params: EchoParams,
        mut results: EchoResults,
    ) -> capnp::capability::Promise<(), capnp::Error> {
        let message = pry!(pry!(pry!(params.get()).get_message()).to_str());
        results.get().set_reply(message);
        Promise::ok(())
    }
}
