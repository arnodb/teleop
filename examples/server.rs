use std::{pin::pin, sync::LazyLock};

use futures::{task::LocalSpawnExt, AsyncReadExt, StreamExt};
use smol_cancellation_token::CancellationToken;
use teleop::{
    attach::unix_socket::listen,
    operate::capnp::{
        self,
        echo::{echo_capnp, EchoServer},
        teleop_capnp, TeleopServer,
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("PID: {}", std::process::id());

    let mut exec = futures::executor::LocalPool::new();
    let spawn = exec.spawner();

    exec.run_until(async {
        let cancellation_token = CancellationToken::new();

        let join_main = std::thread::spawn({
            let cancellation_token = cancellation_token.clone();
            move || -> Result<(), String> {
                std::thread::sleep(std::time::Duration::from_secs(60));
                cancellation_token.cancel();
                Ok(())
            }
        });

        let client = LazyLock::new(|| {
            let mut server = TeleopServer::new();
            server.register_service::<echo_capnp::echo::Client, _, _>("echo", || EchoServer);
            capnp_rpc::new_client::<teleop_capnp::teleop::Client, _>(server)
        });

        let mut conn_stream = pin!(listen(cancellation_token.clone()));
        while let Some(stream) = conn_stream.next().await {
            let (stream, _addr) = stream?;
            if let Err(e) = spawn.spawn_local({
                let cancellation_token = cancellation_token.clone();
                let client = client.client.hook.clone();
                async move {
                    let (input, output) = stream.split();
                    capnp::run_server_connection(input, output, client, cancellation_token).await;
                }
            }) {
                eprintln!("Error while spawning connection handler: {e}");
            }
        }

        join_main
            .join()
            .map_err(|_err| "Unable to join main thread".to_owned())??;

        Ok::<_, Box<dyn std::error::Error>>(())
    })?;

    exec.run();

    Ok(())
}
