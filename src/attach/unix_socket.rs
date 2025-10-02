use std::{fs::File, os::unix::net::SocketAddr, path::PathBuf, time::Duration};

use async_signal::{Signal, Signals};
use async_stream::try_stream;
use futures::{FutureExt, Stream, StreamExt};
use nix::{
    sys::signal::{kill, Signal::SIGQUIT},
    unistd::Pid,
};
use smol::{
    net::unix::{UnixListener, UnixStream},
    Timer,
};
use smol_cancellation_token::CancellationToken;

pub fn listen(
    cancellation_token: CancellationToken,
) -> impl Stream<Item = Result<(UnixStream, SocketAddr), Box<dyn std::error::Error>>> {
    try_stream! {
        await_attach_signal(cancellation_token.clone()).await?;

        if cancellation_token.is_cancelled() {
            return;
        }

        let listener = UnixListener::bind(socket_file_path(std::process::id()))?;

        while let Some(conn) = await_connection(listener.clone(), cancellation_token.clone()).await? {
            yield conn;
        }
    }
}

type AwaitAttachSignalResult = Result<(), Box<dyn std::error::Error>>;

type AwaitConnectionResult = Result<Option<(UnixStream, SocketAddr)>, Box<dyn std::error::Error>>;

async fn await_attach_signal(cancellation_token: CancellationToken) -> AwaitAttachSignalResult {
    let mut signals = Signals::new([Signal::Quit])?;

    loop {
        let mut signal = signals.next().fuse();
        let mut cancelled = cancellation_token.cancelled().fuse();
        futures::select! {
            signal = signal => {
                if let Some(Ok(signal)) = signal {
                    if signal == Signal::Quit {
                        let attach_file_path = attach_file_path(std::process::id());
                        if attach_file_path.exists(){
                            break;
                        }
                    }
                }
            }
            () = cancelled => {
                break;
            }
        }
    }

    Ok(())
}

async fn await_connection(
    listener: UnixListener,
    cancellation_token: CancellationToken,
) -> AwaitConnectionResult {
    let mut accept = Box::pin(listener.accept().fuse());
    let mut cancelled = cancellation_token.cancelled().fuse();
    futures::select! {
        conn = accept => {
            drop(accept);
            Ok(Some(conn?))
        }
        () = cancelled => {
            Ok(None)
        }
    }
}

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
