use std::{pin::pin, sync::LazyLock};

use futures::{task::LocalSpawnExt, AsyncReadExt, StreamExt};
use teleop::{
    attach::unix_socket::listen,
    cancellation::CancellationToken,
    operate::capnp::{
        echo::{echo_capnp, EchoServer},
        run_server_connection, teleop_capnp, TeleopServer,
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pid = std::process::id();
    println!("PID: {pid}");
    if let Ok(pid_file) = std::env::var("PID_FILE") {
        std::fs::write(&pid_file, pid.to_string()).unwrap();
        println!("Wrote it to {pid_file}");
    }

    let mut exec = futures::executor::LocalPool::new();
    let spawn = exec.spawner();

    let res = exec.run_until(async {
        let cancellation_token = CancellationToken::new();

        let join_main = std::thread::spawn({
            let cancellation_token = cancellation_token.clone();
            move || -> Result<(), String> {
                std::thread::sleep(std::time::Duration::from_secs(7));
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
                    run_server_connection(input, output, client, cancellation_token).await;
                }
            }) {
                eprintln!("Error while spawning connection handler: {e}");
            }
        }

        join_main
            .join()
            .map_err(|_err| "Unable to join main thread".to_owned())??;

        Ok::<_, Box<dyn std::error::Error>>(())
    });

    exec.run();

    res?;

    Ok(())
}
