#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{
        pin::pin,
        sync::LazyLock,
        time::{Duration, Instant},
    };

    use async_io::Timer;
    use futures::{task::LocalSpawnExt, AsyncReadExt, FutureExt};
    use teleop::{
        attach::{unix_socket::listen, DefaultAttacher},
        operate::capnp::{
            echo::{echo_capnp, EchoServer},
            run_server_connection, teleop_capnp, TeleopServer,
        },
    };

    let pid = std::process::id();
    println!("PID: {pid}");
    if let Ok(pid_file) = std::env::var("PID_FILE") {
        std::fs::write(&pid_file, pid.to_string()).unwrap();
        println!("Wrote it to {pid_file}");
    }

    let mut exec = futures::executor::LocalPool::new();
    let spawn = exec.spawner();

    let res = exec.run_until(async {
        let mut server_main = Timer::after(Duration::from_secs(7))
            .map(|_: Instant| ())
            .fuse();

        let client = LazyLock::new(|| {
            let mut server = TeleopServer::new();
            server.register_service::<echo_capnp::echo::Client, _, _>("echo", || EchoServer);
            capnp_rpc::new_client::<teleop_capnp::teleop::Client, _>(server)
        });

        let mut conn_stream = pin!(listen::<DefaultAttacher>());
        loop {
            futures::select! {
                stream = futures::StreamExt::next(&mut conn_stream).fuse() => {
                    if let Some(stream) = stream {
                        let (stream, _addr) = stream?;
                        if let Err(e) = spawn.spawn_local({
                            let client = client.client.hook.clone();
                            async move {
                                let (input, output) = stream.split();
                                match run_server_connection(input, output, client).await {
                                    Ok(()) => {}
                                    Err(err) => {
                                        eprintln!("Error while running server connection: {err}");
                                    }
                                }
                            }
                        }) {
                            eprintln!("Error while spawning connection handler: {e}");
                        }
                    } else {
                        break;
                    }
                }
                () = server_main => {
                    break;
                }
            }
        }

        Ok::<_, Box<dyn std::error::Error>>(())
    });

    exec.run();

    res?;

    Ok(())
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{
        pin::pin,
        sync::LazyLock,
        time::{Duration, Instant},
    };

    use async_io::Timer;
    use futures::{task::LocalSpawnExt, AsyncReadExt, FutureExt};
    use teleop::{
        attach::{windows_unix_socket::listen, DefaultAttacher},
        operate::capnp::{
            echo::{echo_capnp, EchoServer},
            run_server_connection, teleop_capnp, TeleopServer,
        },
    };

    let pid = std::process::id();
    println!("PID: {pid}");
    if let Ok(pid_file) = std::env::var("PID_FILE") {
        std::fs::write(&pid_file, pid.to_string()).unwrap();
        println!("Wrote it to {pid_file}");
    }

    let mut exec = futures::executor::LocalPool::new();
    let spawn = exec.spawner();

    let res = exec.run_until(async {
        let mut server_main = Timer::after(Duration::from_secs(7))
            .map(|_: Instant| ())
            .fuse();

        let client = LazyLock::new(|| {
            let mut server = TeleopServer::new();
            server.register_service::<echo_capnp::echo::Client, _, _>("echo", || EchoServer);
            capnp_rpc::new_client::<teleop_capnp::teleop::Client, _>(server)
        });

        let mut conn_stream = pin!(listen::<DefaultAttacher>());
        loop {
            futures::select! {
                stream = futures::StreamExt::next(&mut conn_stream).fuse() => {
                    if let Some(stream) = stream {
                        let (stream, _addr) = stream?;
                        if let Err(e) = spawn.spawn_local({
                            let client = client.client.hook.clone();
                            async move {
                                let (input, output) = stream.split();
                                match run_server_connection(input, output, client).await {
                                    Ok(()) => {}
                                    Err(err) => {
                                        eprintln!("Error while running server connection: {err}");
                                    }
                                }
                            }
                        }) {
                            eprintln!("Error while spawning connection handler: {e}");
                        }
                    } else {
                        break;
                    }
                }
                () = server_main => {
                    break;
                }
            }
        }

        Ok::<_, Box<dyn std::error::Error>>(())
    });

    exec.run();

    res?;

    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
