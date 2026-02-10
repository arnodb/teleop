#[cfg(any(unix, windows))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::env::args;

    use futures::{task::LocalSpawnExt, AsyncReadExt};
    use teleop::{
        attach::{connect, DefaultAttacher},
        operate::capnp::{client_connection, echo::echo_capnp},
    };

    let mut args = args();
    args.next();
    let pid: u32 = args
        .next()
        .unwrap_or_else(|| "PID missing".to_owned())
        .parse()?;

    let mut exec = futures::executor::LocalPool::new();
    let spawn = exec.spawner();

    let res = exec.run_until(async move {
        let stream = connect::<DefaultAttacher>(pid).await?;
        let (input, output) = stream.split();
        let (rpc_system, teleop) = client_connection(input, output).await;
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
}

#[cfg(not(any(unix, windows)))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
