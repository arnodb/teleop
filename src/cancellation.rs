//! Simple cancellation token implementation.
//!
//! Teleop doesn't need a very complex cancellation mechanism, especially not nested tokens. So in
//! order to avoid any extra dependency, here is a very simple implementation.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

struct State {
    cancelled: bool,
    wakers: Vec<Waker>,
}

/// The cancellation token.
///
/// Clone it to listen to the same cancellation event.
#[derive(Clone)]
pub struct CancellationToken {
    state: Arc<Mutex<State>>,
}

impl CancellationToken {
    /// Creates a new cancellation token.
    pub fn new() -> Self {
        CancellationToken {
            state: Arc::new(Mutex::new(State {
                cancelled: false,
                wakers: Vec::new(),
            })),
        }
    }

    /// Signals cancellation and wakes up all the waiters.
    pub fn cancel(&self) {
        let mut state = self.state.lock().unwrap();
        state.cancelled = true;

        // Iterate and wake every future that was pending.
        for waker in state.wakers.drain(..) {
            waker.wake();
        }
    }

    /// Checks the cancellation status synchronously.
    pub fn is_cancelled(&self) -> bool {
        self.state.lock().unwrap().cancelled
    }

    /// Returns a Future that completes when cancellation is requested.
    pub fn cancelled(&self) -> impl Future<Output = ()> + Send + 'static {
        CancelledFuture {
            state: Arc::clone(&self.state),
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

struct CancelledFuture {
    state: Arc<Mutex<State>>,
}

impl Future for CancelledFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock().unwrap();

        if state.cancelled {
            Poll::Ready(())
        } else {
            let current_waker = cx.waker();

            let mut needs_save = true;
            for waker in state.wakers.iter() {
                if current_waker.will_wake(waker) {
                    needs_save = false;
                    break;
                }
            }

            if needs_save {
                state.wakers.push(current_waker.clone());
            }

            Poll::Pending
        }
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn test_cancel_token() {
        let token = CancellationToken::new();
        let token1 = token.clone();

        let th1 = std::thread::spawn(move || {
            block_on(async {
                token1.cancelled().await;
            });
        });

        let th2 = std::thread::spawn(move || {
            token.cancel();
        });

        th1.join().unwrap();
        th2.join().unwrap();
    }
}
