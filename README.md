[![Docs](https://docs.rs/genserver/badge.svg)](https://docs.rs/genserver) [![Crates.io](https://img.shields.io/crates/v/genserver)](https://crates.io/crates/genserver) [![Build & test](https://github.com/brndnmtthws/genserver/actions/workflows/build-and-test.yml/badge.svg)](https://github.com/brndnmtthws/genserver/actions/workflows/build-and-test.yml) [![Codecov](https://img.shields.io/codecov/c/github/brndnmtthws/genserver)](https://app.codecov.io/gh/brndnmtthws/genserver/)

# genserver: generate a server

genserver is tiny async actor framework, inspired by [Elixir's
GenServer](https://hexdocs.pm/elixir/GenServer.html). With few lines code, we do
great things.

Check out [the docs](https://docs.rs/genserver) for more details.

## Synopsis

First run:

```console
$ cargo add genserver
```

Then try this code here in your crate (note the 2 features which need to be
enabled in `main.rs` or `lib.rs` for applications and libraries respectively):

```rust
// these two features must be enabled at the crate level
#![feature(generic_associated_types, type_alias_impl_trait)]

use std::future::Future;

use genserver::{make_registry, GenServer};

struct MyServer {
    // Any state your server needs can go right
    // in here. We keep a registry around in case
    // we want to call any other servers from our
    // server.
    registry: MyRegistry,
}

impl GenServer for MyServer {
    // Message type for this server.
    type Message = String;
    // The type of the registry defined by the `make_registry` attribute macro.
    type Registry = MyRegistry;
    // The response type for calls.
    type Response = String;

    // The call response type, a future, which returns Self::Response.
    type CallResponse<'a> = impl Future<Output = Self::Response> + 'a;
    // The cast response type, also a future, which returns unit.
    type CastResponse<'a> = impl Future<Output = ()> + 'a;

    fn new(registry: Self::Registry) -> Self {
        // When are server is started, the registry passes a copy
        // of itself here. We keep it around so we can call
        // other servers from this one.
        Self { registry }
    }

    // Calls to handle_call will block until our `Response` is returned.
    // Because they return a future, we can return an async block here.
    fn handle_call(&mut self, message: Self::Message) -> Self::CallResponse<'_> {
        println!("handle_call received {}", message);
        std::future::ready("returned from handle_call".into())
    }

    // Casts always return (), because they do not block callers and return
    // immediately.
    fn handle_cast(&mut self, message: Self::Message) -> Self::CastResponse<'_> {
        println!("handle_cast received {}", message);
        std::future::ready(())
    }
}

#[make_registry{
    myserver: MyServer
}]
struct MyRegistry;

tokio_test::block_on(async {
    let registry = MyRegistry::start().await;

    let response = registry
        .call_myserver("calling myserver".into())
        .await
        .unwrap();
    registry
        .cast_myserver("casting to myserver".into())
        .await
        .unwrap();
});
```
