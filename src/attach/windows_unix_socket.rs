//! Communicate through a Windows UNIX socket.
//!
//! [`listen`] is the function to call in the process to be teleoperated.
//!
//! [`connect`] is the function to call in the client to initiate the teleoperation communication.

use std::{
    ops::Deref,
    os::windows::{
        io::AsRawSocket,
        prelude::{AsSocket, BorrowedSocket},
    },
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};

use async_io::{Async, Timer};
use async_stream::try_stream;
use futures::{
    task::{Context, Poll},
    AsyncRead, AsyncWrite, Stream,
};
use uds_windows::{SocketAddr, UnixListener, UnixStream};

use crate::attach::attacher::{Attacher, AttacherSignal};

#[derive(Debug)]
struct UdsListenerWrapper(UnixListener);

impl Deref for UdsListenerWrapper {
    type Target = UnixListener;

    fn deref(&self) -> &UnixListener {
        &self.0
    }
}

impl AsSocket for UdsListenerWrapper {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
    }
}

#[derive(Debug)]
pub struct UdsStream(Async<UnixStream>);

impl AsyncRead for UdsStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let pinned = std::pin::pin!(&self.0);
        pinned.poll_read(cx, buf)
    }
}

impl AsyncWrite for UdsStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let pinned = std::pin::pin!(&self.0);
        pinned.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        let pinned = std::pin::pin!(&self.0);
        pinned.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        let pinned = std::pin::pin!(&self.0);
        pinned.poll_close(cx)
    }
}

/// Starts listening for attach signals and return incoming connections as a async `Stream`.
///
/// In order to stop accepting connections, it is enough to stop polling the stream.
pub fn listen<A>() -> impl Stream<Item = Result<(UdsStream, SocketAddr), Box<dyn std::error::Error>>>
where
    A: Attacher,
{
    // It is important to keep this in the synchronous part in order to ensure the listening
    // process is ready to accept attachment requests even if the future is not awaited.
    //
    // Nevertheless, the error will only be raised if the future is awaited.
    let signaled = A::signaled();

    try_stream! {

        signaled.await?;

        let listener = Async::new(
            UdsListenerWrapper(
                UnixListener::bind(socket_file_path(std::process::id()))?
            )
        )?;

        loop {
            let (stream, addr) = listener.read_with(|l| l.accept()).await?;
            yield (UdsStream(Async::new(stream)?), addr);
        }
    }
}

/// Connects to a process identified by its ID.
///
/// Returns the opened socket on success.
pub async fn connect<A>(pid: u32) -> Result<UdsStream, Box<dyn std::error::Error>>
where
    A: Attacher,
{
    let socket_file_path = socket_file_path(pid);
    connect_to_socket::<A>(pid, &socket_file_path).await
}

pub async fn connect_to_socket<A>(
    pid: u32,
    socket_file_path: impl AsRef<Path>,
) -> Result<UdsStream, Box<dyn std::error::Error>>
where
    A: Attacher,
{
    let socket_file_path = socket_file_path.as_ref();

    if !socket_file_path.exists() {
        let mut signal = A::signal(pid)?;

        signal.send().await?;

        let mut attempts = 1;

        while !socket_file_path.exists() && attempts < 100 {
            Timer::after(Duration::from_millis(100)).await;

            signal.send().await?;

            attempts += 1;
        }

        if !socket_file_path.exists() {
            return Err(format!(
                "Unable to open socket file {}: target process {} doesn't respond",
                socket_file_path.to_string_lossy(),
                pid
            )
            .into());
        }
    }

    Ok(UdsStream(Async::new(UnixStream::connect(
        socket_file_path,
    )?)?))
}

fn socket_file_path(pid: u32) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(".teleop_pid_{pid}"));
    path
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::pin::pin;

    use assert_matches::assert_matches;
    use futures::{
        channel::oneshot,
        io::{BufReader, BufWriter},
        AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, StreamExt,
    };

    use super::*;
    use crate::{
        attach::attacher::{dummy::DummyAttacher, DefaultAttacher},
        tests::ATTACH_PROCESS_TEST_MUTEX,
    };

    fn socket_file_path_for_failure(pid: u32) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(".teleop_pid_{pid}_fail"));
        path
    }

    #[test]
    fn test_unix_socket_attachment() {
        // This test may conflict with attacher tests
        let _attacher_test = ATTACH_PROCESS_TEST_MUTEX.lock();

        let (sender, receiver) = oneshot::channel::<()>();

        let server = || -> Result<(), Box<dyn std::error::Error>> {
            let mut exec = futures::executor::LocalPool::new();

            let res = exec.run_until(async {
                let mut conn_stream = pin!(listen::<DefaultAttacher>());
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
                let stream = connect::<DefaultAttacher>(pid).await?;
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
        // Improve code coverage by letting the server avoid early returns
        std::thread::sleep(Duration::from_secs(2));
        let c = std::thread::spawn(|| client().unwrap());
        c.join().unwrap();
        s.join().unwrap();
    }

    #[test]
    fn test_unix_socket_attachment_failure() {
        // This test may not conflict with the other tests because
        // * it uses the dummy attacher
        // * it uses a special socket path

        let client = || -> Result<(), Box<dyn std::error::Error>> {
            let pid = std::process::id();

            let mut exec = futures::executor::LocalPool::new();

            let res = exec.run_until(async move {
                let result =
                    connect_to_socket::<DummyAttacher>(pid, socket_file_path_for_failure(pid))
                        .await;
                let err = assert_matches!(result, Err(err) => err);
                assert!(
                    err.to_string().starts_with("Unable to open socket file"),
                    "Expected error `{err}` to start with `Unable to open socket file`."
                );
                Ok::<_, Box<dyn std::error::Error>>(())
            });

            exec.run();

            res?;

            Ok(())
        };

        client().unwrap();
    }
}
