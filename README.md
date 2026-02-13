# Teleop
[![Latest Version](https://img.shields.io/crates/v/teleop)](https://crates.io/crates/teleop)
[![Documentation](https://docs.rs/teleop/badge.svg)](https://docs.rs/teleop)
[![Build Status](https://github.com/arnodb/teleop/actions/workflows/ci.yml/badge.svg)](https://github.com/arnodb/teleop/actions/workflows/ci.yml)
[![Code Coverage](https://codecov.io/gh/arnodb/teleop/branch/main/graph/badge.svg)](https://codecov.io/gh/arnodb/teleop)

Teleop provides means to attach to a local process knowing its ID, and then provides RPC capabilities to the client.

## Attachers

|**Attacher**|**Platform**|**Feature**|**Comment**|
|-|-|-|-|
| Inotify ([inotify](https://crates.io/crates/inotify)) | <ul><li>`linux`</li><li>any platform where `inotify` compiles</li></ul> | `inotify` | It monitors a specific file before binding the communication channel.<br><br> It is the default when the feature is enabled. |
| Kqueue ([kqueue](https://crates.io/crates/kqueue)) | <ul><li>`target_os = "macos"`</li></ul> | Always included on supported platforms | It monitors a specific file before binding the communication channel.<br><br> It is the default on `target_os = "macos"`. |
| Unix | <ul><li>`unix`</li></ul> | Always included on supported platforms | It waits for a signal, checks the existence of a specific file and then binds the communication channel.<br><br> Quite outdated in 2025. |
| Dummy | All platforms | Always included on supported platforms | The communication channel is immediately bound.<br><br> It is the default when no other option is available (e.g. on `windows`) |

Unfortunately, `async-io` does not provide yet support to monitor directory changes on Windows. Maintainer of Teleop is open to any suggestion on the matter.

Kqueue is likely supported on other platforms but not in Teleop until it is proved to work (via CI). Feel free to open PRs to fine tune the platform guards and the CI jobs.

## Communication channels

|**Communication channel**|**Platform**|**Comment**|
|-|-|-|
|UNIX socket ([async-net](https://crates.io/crates/async-net) - smol) | <ul><li>`unix`</li></ul> | Regular UNIX socket. |
|Windows UNIX socket ([uds_windows](https://crates.io/crates/uds_windows)) | <ul><li>`windows`</li></ul> | Windows UNIX socket. |

Unfortunately, `async-io` does not support Windows named pipes yet. It is assumed that the UNIX socket on Windows is a good start.

## Operations protocol

Teleop supports only Cap’n Proto RPC, but it is designed such as more ways to operate a process could be provided.

### Cap'n Proto RPC

Teleop provides a root interface named `Teleop` (see `teleop.capnp`) which gives access to arbitrary services.

## Process discovery

At this time, the process discovery is very likely to remain a per app process for the following reasons...

As opposed to Java Console, the client side is not aimed at teleoperating "any" app that has teleop enabled.

Apps likely want to convey more information during the discovery process than just "hey, I'm here". If one takes the Java example, the discovery is performed via the performance data file each process creates (unless told not to).

That is why Teleop provides nothing regarding process discovery. One can see the Quirky Binder use case below which has very basic discovery mechanism.

Happy to revisit the issue later.

## Example

* [server.rs](examples/server.rs) shows how to setup the process to teleoperate, including an `echo` service which will reply to a request by echoing the input.
* [client.rs](examples/client.rs) shows how to setup the client, initiate the attach process, request the `echo` service, and send echo requests.

## Use cases

* [quirky_binder](https://github.com/arnodb/quirky_binder) uses it to enable quirky teleoperations like inspecting the current state of data processing.
* [quirky_binder_console](https://github.com/arnodb/quirky_binder_console) uses it to connect to a Quirky Binder process (that has teleop enabled) and display nice SVGs.

## Where does this come from?

The implementation is very much inspired by Java [Attach API](https://docs.oracle.com/javase/8/docs/technotes/guides/attach/index.html):

* the process to be teleoperated waits for a signal
* if some conditions are met then it opens the UNIX socket at a known location
* the client can then connect to the UNIX socket and use the RPC protocol set up by the remote process

