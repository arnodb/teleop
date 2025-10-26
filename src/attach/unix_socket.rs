//! Attach API using a UNIX socket.
//!
//! It is very much inspired by Java [Attach API](https://docs.oracle.com/javase/8/docs/technotes/guides/attach/index.html):
//! * the process to be teleoperated waits for a signal
//! * if some conditions are met then it opens the UNIX socket at a known location
//! * the client can then connect to the unix socket
//!
//! [`listen`] is the function to call in the process to be teleoperated.
//!
//! [`connect`] is the function to call in the client to initiate the teleoperation communication.

use std::{fs::File, future::Future, os::unix::net::SocketAddr, path::PathBuf, time::Duration};

use async_signal::{Signal, Signals};
use async_stream::try_stream;
use futures::{Stream, StreamExt};
use nix::{
    sys::signal::{kill, Signal::SIGQUIT},
    unistd::Pid,
};
use smol::{
    net::unix::{UnixListener, UnixStream},
    Timer,
};

/// Starts listening for attach signals and return incoming connections as a async `Stream`.
///
/// In order to stop accepting connections, it is enough to stop polling the stream.
pub fn listen() -> impl Stream<Item = Result<(UnixStream, SocketAddr), Box<dyn std::error::Error>>>
{
    // It is important to keep this in the synchronous part in order to ensure the listening
    // process is ready to accept attachment requests even if the future is not awaited.
    //
    // Nevertheless, the error will only be raised if the future is awaited.
    let signal_attached = await_attach_signal();

    try_stream! {
        signal_attached.await?;

        let listener = UnixListener::bind(socket_file_path(std::process::id()))?;

        loop {
            let conn = listener.accept().await?;
            yield conn;
        }
    }
}

type AwaitAttachSignalResult = Result<(), Box<dyn std::error::Error>>;

fn await_attach_signal() -> impl Future<Output = AwaitAttachSignalResult> {
    // It is important to keep this in the synchronous part in order to ensure the listening
    // process is ready to accept attachment requests even if the future is not awaited.
    //
    // Nevertheless, the error will only be raised if the future is awaited.
    let signals = Signals::new([Signal::Quit]);

    async move {
        let mut signals = signals?;

        while let Some(signal) = signals.next().await {
            if let Ok(signal) = signal {
                if signal == Signal::Quit {
                    let attach_file_path = attach_file_path(std::process::id());
                    if attach_file_path.exists() {
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

/// Connects to a process identified by its ID.
///
/// Returns the opened socket on success.
pub async fn connect(pid: u32) -> Result<UnixStream, Box<dyn std::error::Error>> {
    let socket_file_path = socket_file_path(pid);

    if !socket_file_path.exists() {
        let _attach_file: AutoDropFile = AutoDropFile::create(attach_file_path(pid))?;

        kill(Pid::from_raw(pid as _), SIGQUIT)?;

        let mut attempts = 1;

        while !socket_file_path.exists() && attempts < 100 {
            Timer::after(Duration::from_millis(100)).await;

            kill(Pid::from_raw(pid as _), SIGQUIT)?;

            attempts += 1;
        }

        if !socket_file_path.exists() {
            panic!(
                "Unable to open socket file {}: target process {} doesn't respond",
                socket_file_path.to_string_lossy(),
                pid
            );
        }
    }

    Ok(UnixStream::connect(socket_file_path).await?)
}

fn attach_file_path(pid: u32) -> PathBuf {
    let mut path = PathBuf::new();
    path.push("/proc");
    path.push(pid.to_string());
    path.push("cwd");
    path.push(format!(".teleop_attach_{pid}"));
    path
}

fn socket_file_path(pid: u32) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(".teleop_pid_{pid}"));
    path
}

struct AutoDropFile(PathBuf);

impl AutoDropFile {
    pub fn create(path: PathBuf) -> std::io::Result<Self> {
        File::create(&path)?;
        Ok(Self(path))
    }
}

impl Drop for AutoDropFile {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_file(&self.0).unwrap();
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::pin::pin;

    use futures::{
        channel::oneshot,
        io::{BufReader, BufWriter},
        AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, StreamExt,
    };

    use super::*;

    #[test]
    fn test_unix_socket_attachment() {
        let (sender, receiver) = oneshot::channel::<()>();

        let server = || -> Result<(), Box<dyn std::error::Error>> {
            let mut exec = futures::executor::LocalPool::new();

            let res = exec.run_until(async {
                let mut conn_stream = pin!(listen());
                println!("server is listening");
                sender.send(()).unwrap();
                if let Some(stream) = conn_stream.next().await {
                    println!("server received connection");
                    let (stream, _addr) = stream?;
                    let (input, output) = stream.split();
                    let mut input = BufReader::new(input);
                    let mut output = BufWriter::new(output);

                    let mut read = String::new();
                    while input.read_line(&mut read).await? == 0 {}
                    assert_eq!(read, "ping\n");
                    println!("server received ping");

                    output.write_all("pong\n".as_bytes()).await?;
                    output.flush().await?;
                    println!("server wrote pong");
                }

                Ok::<_, Box<dyn std::error::Error>>(())
            });

            exec.run();

            res?;

            Ok(())
        };

        let client = || -> Result<(), Box<dyn std::error::Error>> {
            let pid = std::process::id();

            let mut exec = futures::executor::LocalPool::new();

            let res = exec.run_until(async move {
                let () = receiver.await?;
                println!("client is initiating connection");
                let stream = connect(pid).await?;
                let (input, output) = stream.split();
                let mut input = BufReader::new(input);
                let mut output = BufWriter::new(output);
                println!("client is connected");
                output.write_all("ping\n".as_bytes()).await?;
                output.flush().await?;
                println!("client wrote ping");

                let mut read = String::new();
                while input.read_line(&mut read).await? == 0 {}
                assert_eq!(read, "pong\n");
                println!("client received pong");

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
