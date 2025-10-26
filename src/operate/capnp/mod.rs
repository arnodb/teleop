//! Cap'n Proto RPC capabilities.
//!
//! [`TeleopServer`] is the structure to create the main Teleop server and set it up with
//! predefined services.
//!
//! [`run_server_connection`] is called to wire some communication streams with a [`TeleopServer`]
//! and operate the entire stack.
//!
//! [`client_connection`] is called to wire some communication streams and expose a `Teleop` client
//! endpoint.

use std::{collections::BTreeMap, sync::LazyLock};

use capnp::{
    capability::{Client, FromClientHook, FromServer, Promise},
    private::capability::ClientHook,
    Error,
};
use capnp_rpc::{pry, rpc_twoparty_capnp, twoparty, RpcSystem};
use futures::{
    io::{BufReader, BufWriter},
    AsyncRead, AsyncWrite,
};

pub mod echo;

capnp::generated_code!(pub mod teleop_capnp);

/// Main structure to start teleoperations with Cap'n Proto RPC.
#[derive(Default)]
pub struct TeleopServer {
    #[allow(clippy::type_complexity)]
    services:
        BTreeMap<String, LazyLock<Box<dyn ClientHook>, Box<dyn FnOnce() -> Box<dyn ClientHook>>>>,
}

impl TeleopServer {
    /// Creates a new server with no services registered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new service, lazily initialized via the passed callback.
    ///
    /// The service is not initialized until it is requested by a client.
    pub fn register_service<Client, Server, F>(&mut self, name: impl Into<String>, f: F)
    where
        Client: FromClientHook + FromServer<Server>,
        F: FnOnce() -> Server + 'static,
    {
        self.services.insert(
            name.into(),
            LazyLock::new(Box::new(|| {
                let client: Client = capnp_rpc::new_client(f());
                Box::<dyn ClientHook>::new(client.into_client_hook())
            })),
        );
    }
}

impl teleop_capnp::teleop::Server for TeleopServer {
    fn service(
        &mut self,
        params: teleop_capnp::teleop::ServiceParams,
        mut results: teleop_capnp::teleop::ServiceResults,
    ) -> Promise<(), Error> {
        let name = pry!(pry!(pry!(params.get()).get_name()).to_str());
        let service = self.services.get(name);
        if let Some(service) = service {
            results
                .get()
                .init_service()
                .set_as_capability((*service).clone());
            Promise::ok(())
        } else {
            Promise::err(Error::failed(format!("service {name} not found")))
        }
    }
}

/// Runs a new RPC server connection.
///
/// The communication goes through the passed input and output.
///
/// The Cap'n Proto main service is passed as an abstract `capnp` client.
///
/// The connection can be cancelled with the passed cancellation token.
pub async fn run_server_connection<R, W>(
    input: R,
    output: W,
    client: Box<dyn ClientHook>,
) -> Result<(), capnp::Error>
where
    R: AsyncRead + Unpin + 'static,
    W: AsyncWrite + Unpin + 'static,
{
    let network = twoparty::VatNetwork::new(
        BufReader::new(input),
        BufWriter::new(output),
        rpc_twoparty_capnp::Side::Server,
        Default::default(),
    );
    let rpc_system = RpcSystem::new(Box::new(network), Some(Client { hook: client }));

    rpc_system.await
}

/// Creates a RPC client connection.
///
/// The communication goes through the passed input and output.
///
/// The returned value is made of a system to be run by the async runtime and the client interface
/// to initiate RPC requests.
pub async fn client_connection<R, W>(
    input: R,
    output: W,
) -> (
    RpcSystem<rpc_twoparty_capnp::Side>,
    teleop_capnp::teleop::Client,
)
where
    R: AsyncRead + Unpin + 'static,
    W: AsyncWrite + Unpin + 'static,
{
    let network = twoparty::VatNetwork::new(
        BufReader::new(input),
        BufWriter::new(output),
        rpc_twoparty_capnp::Side::Client,
        Default::default(),
    );
    let mut rpc_system = RpcSystem::new(Box::new(network), None);
    let teleop: teleop_capnp::teleop::Client =
        rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);
    (rpc_system, teleop)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {

    use futures::task::LocalSpawnExt;

    use super::{
        echo::{echo_capnp, EchoServer},
        *,
    };

    #[test]
    fn test_capnp_teleop() {
        let (client_input, server_output) = sluice::pipe::pipe();
        let (server_input, client_output) = sluice::pipe::pipe();

        let server = || -> Result<(), Box<dyn std::error::Error>> {
            let mut server = TeleopServer::new();
            server.register_service::<echo_capnp::echo::Client, _, _>("echo", || EchoServer);
            let client = capnp_rpc::new_client::<teleop_capnp::teleop::Client, _>(server);

            let mut exec = futures::executor::LocalPool::new();

            let res = exec.run_until(run_server_connection(
                server_input,
                server_output,
                client.client.hook,
            ));

            exec.run();

            res?;

            Ok(())
        };

        let client = || -> Result<(), Box<dyn std::error::Error>> {
            let mut exec = futures::executor::LocalPool::new();
            let spawn = exec.spawner();

            let res = exec.run_until(async move {
                let (rpc_system, teleop) = client_connection(client_input, client_output).await;
                let rpc_disconnect = rpc_system.get_disconnector();

                spawn.spawn_local(async {
                    if let Err(e) = rpc_system.await {
                        eprintln!("Connection interrupted {e}");
                    }
                })?;

                let res = async {
                    let mut req = teleop.service_request();
                    req.get().set_name("echo");
                    let echo = req.send().promise.await?;
                    let echo = echo.get()?.get_service();
                    let echo: echo_capnp::echo::Client = echo.get_as()?;

                    println!("got echo service");

                    let mut req = echo.echo_request();
                    req.get().set_message("hello!");
                    let reply = req.send().promise.await?;
                    let reply = reply.get()?.get_reply()?.to_str()?;

                    println!("{}", reply);

                    let mut req = teleop.service_request();
                    req.get().set_name("tango");
                    let tango_res = req.send().promise.await;
                    assert!(tango_res.is_err());
                    let tango_err = tango_res.err().unwrap();
                    assert_eq!(tango_err.kind, capnp::ErrorKind::Failed);
                    assert!(tango_err.extra.contains("service tango not found"));

                    Ok::<_, Box<dyn std::error::Error>>(())
                }
                .await;

                let res2 = rpc_disconnect.await;

                res?;

                res2?;

                Ok::<_, Box<dyn std::error::Error>>(())
            });

            exec.run();

            res?;

            Ok(())
        };

        let s = std::thread::spawn(|| server().unwrap());
        let c = std::thread::spawn(|| client().unwrap());
        c.join().unwrap();
        s.join().unwrap();
    }
}
