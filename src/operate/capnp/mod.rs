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
    AsyncRead, AsyncWrite, FutureExt,
};

use crate::cancellation::CancellationToken;

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
            Promise::err(Error::unimplemented(format!("service {name} not found")))
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
    cancellation_token: CancellationToken,
) where
    R: AsyncRead + Unpin + 'static,
    W: AsyncWrite + Unpin + 'static,
{
    let network = twoparty::VatNetwork::new(
        BufReader::new(input),
        BufWriter::new(output),
        rpc_twoparty_capnp::Side::Server,
        Default::default(),
    );
    let mut rpc_system = RpcSystem::new(Box::new(network), Some(Client { hook: client })).fuse();

    let mut cancelled = cancellation_token.cancelled().fuse();
    futures::select! {
        _ = rpc_system => {
            eprintln!("Connection interrupted");
        }
        () = cancelled => {
            eprintln!("Process interrupted");
        }
    }
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
