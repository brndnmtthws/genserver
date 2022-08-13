//! # genserver: generate a server
//!
//! This is a neat little create for building async actor-based applications,
//! inspired by Elixir's GenServer, powered by Tokio.
//!
//! This crate is currently nightly-only, and requires unstable features
//! (`#![feature(type_alias_impl_trait, generic_associated_types)]`).
//!
//! ## Introduction
//!
//! Erlang's OTP has reached legend amongst computer people for its rock solid
//! reliability, super cool concurrency, failure handling, hot code reloading,
//! and web scale innovation. [This excellent documentary
//! film](https://www.youtube.com/watch?v=rRbY3TMUcgQ) provides a good overview of
//! some of the coolness of Erlang.
//!
//! This crate has nothing to do Erlang, but it takes inspiration from Elixir
//! (Erlang's younger, ever more hip successor) and its GenServer. Here we
//! provide a method and interface for creating very simple actors which can
//! call other actors and reply to messages.
//!
//! Due to some quirks of the current Rust async implementation, this crate has
//! to do a few janky things to make everything work nice, however the end
//! result is a nice little API for building legendary actor-based systems. For
//! example, Rust doesn't yet support async methods in traits, but we can
//! provide similar behaviour by enabling a few unstable features.
//!
//! Underneath the hood, Tokio is leveraged for its sweet concurrency
//! primitives. You can generate as many servers as you'd like, and each is
//! spawned into its own async context (like a thread, but not exactly).
//!
//! We can build some really cool stuff using this very simple abstraction. And
//! since it all runs within an async context, we can build super fast web scale
//! technology in no time. Plus we get all the benefits of Rust, especially the
//! bragging rights.
//!
//! For more usage examples (aside from what's in the docs), check out the tests
//! in [`tests/test.rs`](https://github.com/brndnmtthws/genserver/blob/main/tests/test.rs)
//! within this crate.
//!
//! ## Quickstart
//!
//! To get started, you'll need to do 3 things:
//!
//! * Define one or more servers, which respond to _calls_ and _casts_, and
//!   implement the [`GenServer`] trait.
//! * Define a registry with [`make_registry`], which starts and managers the
//!   servers you define
//! * Enable the `generic_associated_types` and `type_alias_impl_trait` features
//!   at the crate level.
//!
//! When we make a _call_, the call will block until it returns a result from
//! the server. When we make a _cast_, the cast returns immediately after it's
//! dispatched to the server.
//!
//! We need to start our registry within an async context. Refer
//!
//! Here is a minimal code example showing a server, `MyServer`, which
//! implements [`GenServer`]. We then create a registry and start it.
//!
//! ```
//! // these two features must be enabled at the crate level
//! #![feature(generic_associated_types, type_alias_impl_trait)]
//!
//! use std::future::Future;
//!
//! use genserver::{make_registry, GenServer};
//!
//! struct MyServer {
//!     // Any state your server needs can go right
//!     // in here. We keep a registry around in case
//!     // we want to call any other servers from our
//!     // server.
//!     registry: MyRegistry,
//! }
//!
//! impl GenServer for MyServer {
//!     // Message type for this server.
//!     type Message = String;
//!     // The type of the registry defined by the `make_registry` attribute macro.
//!     type Registry = MyRegistry;
//!     // The response type for calls.
//!     type Response = String;
//!
//!     // The call response type, a future, which returns Self::Response.
//!     type CallResponse<'a> = impl Future<Output = Self::Response> + 'a;
//!     // The cast response type, also a future, which returns unit.
//!     type CastResponse<'a> = impl Future<Output = ()> + 'a;
//!
//!     fn new(registry: Self::Registry) -> Self {
//!         // When are server is started, the registry passes a copy
//!         // of itself here. We keep it around so we can call
//!         // other servers from this one.
//!         Self { registry }
//!     }
//!
//!     // Calls to handle_call will block until our `Response` is returned.
//!     // Because they return a future, we can return an async block here.
//!     fn handle_call(&mut self, message: Self::Message) -> Self::CallResponse<'_> {
//!         println!("handle_call received {}", message);
//!         std::future::ready("returned from handle_call".into())
//!     }
//!
//!     // Casts always return (), because they do not block callers and return
//!     // immediately.
//!     fn handle_cast(&mut self, message: Self::Message) -> Self::CastResponse<'_> {
//!         println!("handle_cast received {}", message);
//!         std::future::ready(())
//!     }
//! }
//!
//! #[make_registry{
//!     myserver: MyServer
//! }]
//! struct MyRegistry;
//!
//! tokio_test::block_on(async {
//!     let registry = MyRegistry::start().await;
//!
//!     let response = registry
//!         .call_myserver("calling myserver".into())
//!         .await
//!         .unwrap();
//!     registry
//!         .cast_myserver("casting to myserver".into())
//!         .await
//!         .unwrap();
//! });
//! ```
//!
//! Note that in the code above, `MyServer::handle_call` and
//! `MyServer::handle_cast` return futures. That means you can include an async
//! block within the function and return it. Refer to [`GenServer`] for more
//! details.
//!
//! ## Supplying initialization data to your server
//!
//! With Elixir's GenServer, you can initialize state with the `init()` method.
//! However, in Rust this is a bit tricky due to the type rules, and it makes
//! the implementation much more complicated.
//!
//! The [`GenServer`] trait lets you control the initialization of your server
//! with the `new()` method. However, there's no way to pass additional
//! parameters to `new()`. Instead, you have 2 options:
//!
//! 1. You can simply create a message with initial parameters, and send that
//! message to your server immediately after launching. This is the best option,
//! though it has some drawbacks such as needing to make most fields optional.
//! 2. You could add fields to your [`Registry`]. This is the least preferred
//! option, as the registry should be immutable and easy to make many copies of.
//!
//! ## Changing channel queue size
//!
//! We use bounded queues, which provide backpressure when queues are full. This
//! is probably not something you'll need to worry much about, but if (for some
//! reason) you want to change the channel queue length, it can be done so by
//! implementing the [`GenServer::channel_queue_size()`] method for your server.
#![feature(generic_associated_types)]

use std::future::Future;

/// Makes a registry.
///
/// This attribute applies to structs and converts an ordinary struct into a
/// registry. While you can pass any ordinary struct to this attribute, it's
/// recommended that you don't add any fields or methods to the registry crate.
///
/// You must specify the servers you want to register as an argument, in `name:
/// Type` pairs.
///
/// For example, if we use the following code:
///
/// ```compile_fail
/// #[make_registry {
///     first_server: FirstServer,
///     second_server: SecondServer,
/// }]
/// struct MyRegistry;
/// ```
///
/// This will generate a registry called `MyRegistry` which implements the
/// [`Registry`] trait. In the example above, we specified 2 servers,
/// named `first_server` and `second_server`, and are of the type `FirstServer`
/// and `SecondServer` respectively.
///
/// For each server registered with this registry, the `call_{name}()`,
/// `call_{name}_with_timeout()`, and `cast_{name}()` will be generated for this
/// registry which can be used to make calls and casts to each server. You can
/// (if you wish) define multiple separate instances of the same type, so long
/// as each one has a unique name, much like you would when defining struct
/// fields.
///
/// Using the example above, we'd generate functions with the following names in
/// our registry:
///
/// | Call fn | Call fn with timeout | Cast fn (non-blocking) |
/// |---|---|---|
/// |`call_first_server`|`call_first_server_with_timeout`|`cast_first_server`|
/// |`call_second_server`|`call_second_server_with_timeout`|`cast_second_server`|
///
/// This macro will additionally derive the [`Clone`] trait.
///
/// Note that this example will not compile because the [`GenServer`] trait is
/// not implemented for this example case.
///
/// The full generated code for the example above is as follows:
///
/// ```compile_fail
/// struct MyRegistry {
///     first_server_tx: tokio::sync::mpsc::Sender<(
///         <FirstServer as genserver::GenServer>::Message,
///         Option<tokio::sync::oneshot::Sender<<FirstServer as genserver::GenServer>::Response>>,
///     )>,
///     second_server_tx: tokio::sync::mpsc::Sender<(
///         <SecondServer as genserver::GenServer>::Message,
///         Option<tokio::sync::oneshot::Sender<<SecondServer as genserver::GenServer>::Response>>,
///     )>,
/// }
/// #[automatically_derived]
/// impl ::core::clone::Clone for MyRegistry {
///     #[inline]
///     fn clone(&self) -> MyRegistry {
///         MyRegistry {
///             first_server_tx: ::core::clone::Clone::clone(&self.first_server_tx),
///             second_server_tx: ::core::clone::Clone::clone(&self.second_server_tx),
///         }
///     }
/// }
/// impl genserver::Registry for MyRegistry {}
/// impl MyRegistry {
///     pub async fn start() -> Self {
///         let (first_server_tx, mut first_server_rx) = tokio::sync::mpsc::channel::<(
///             <FirstServer as genserver::GenServer>::Message,
///             Option<
///                 tokio::sync::oneshot::Sender<<FirstServer as genserver::GenServer>::Response>,
///             >,
///         )>(1_000);
///         let (second_server_tx, mut second_server_rx) = tokio::sync::mpsc::channel::<(
///             <SecondServer as genserver::GenServer>::Message,
///             Option<
///                 tokio::sync::oneshot::Sender<<SecondServer as genserver::GenServer>::Response>,
///             >,
///         )>(1_000);
///         let mut registry = Self {
///             first_server_tx,
///             second_server_tx,
///         };
///         {
///             let local_registry = registry.clone();
///             tokio::spawn(async move {
///                 let mut handler = FirstServer::new(local_registry);
///                 while let Some((message, oneshot)) = first_server_rx.recv().await {
///                     if let Some(oneshot) = oneshot {
///                         let Response = handler.handle_call(message).await;
///                         oneshot.send(Response).ok();
///                     } else {
///                         handler.handle_cast(message).await;
///                     }
///                 }
///             });
///         }
///         {
///             let local_registry = registry.clone();
///             tokio::spawn(async move {
///                 let mut handler = SecondServer::new(local_registry);
///                 while let Some((message, oneshot)) = second_server_rx.recv().await {
///                     if let Some(oneshot) = oneshot {
///                         let Response = handler.handle_call(message).await;
///                         oneshot.send(Response).ok();
///                     } else {
///                         handler.handle_cast(message).await;
///                     }
///                 }
///             });
///         }
///         registry
///     }
///
///     pub async fn call_first_server(
///         &self,
///         message: <FirstServer as genserver::GenServer>::Message,
///     ) -> Result<
///         <FirstServer as genserver::GenServer>::Response,
///         genserver::Error<(
///             <FirstServer as genserver::GenServer>::Message,
///             Option<
///                 tokio::sync::oneshot::Sender<<FirstServer as genserver::GenServer>::Message>,
///             >,
///         )>,
///     > {
///         let (oneshot_tx, oneshot_rx) =
///             tokio::sync::oneshot::channel::<<FirstServer as genserver::GenServer>::Response>();
///         self.first_server_tx
///             .send((message, Some(oneshot_tx)))
///             .await?;
///         let Response = oneshot_rx.await?;
///         Ok(Response)
///     }
///
///     pub async fn cast_first_server(
///         &self,
///         message: <FirstServer as genserver::GenServer>::Message,
///     ) -> Result<
///         (),
///         genserver::Error<(
///             <FirstServer as genserver::GenServer>::Message,
///             Option<
///                 tokio::sync::oneshot::Sender<<FirstServer as genserver::GenServer>::Message>,
///             >,
///         )>,
///     > {
///         self.first_server_tx.send((message, None)).await?;
///         Ok(())
///     }
///
///     pub async fn call_second_server(
///         &self,
///         message: <SecondServer as genserver::GenServer>::Message,
///     ) -> Result<
///         <SecondServer as genserver::GenServer>::Response,
///         genserver::Error<(
///             <SecondServer as genserver::GenServer>::Message,
///             Option<
///                 tokio::sync::oneshot::Sender<<SecondServer as genserver::GenServer>::Message>,
///             >,
///         )>,
///     > {
///         let (oneshot_tx, oneshot_rx) =
///             tokio::sync::oneshot::channel::<<SecondServer as genserver::GenServer>::Response>();
///         self.second_server_tx
///             .send((message, Some(oneshot_tx)))
///             .await?;
///         let Response = oneshot_rx.await?;
///         Ok(Response)
///     }
///
///     pub async fn cast_second_server(
///         &self,
///         message: <SecondServer as genserver::GenServer>::Message,
///     ) -> Result<
///         (),
///         genserver::Error<(
///             <SecondServer as genserver::GenServer>::Message,
///             Option<
///                 tokio::sync::oneshot::Sender<<SecondServer as genserver::GenServer>::Message>,
///             >,
///         )>,
///     > {
///         self.second_server_tx.send((message, None)).await?;
///         Ok(())
///     }
/// }
/// ```
///
/// ## Adding your own state
///
/// If you'd like to add your own state to a registry, you can do so like you
/// normally would with any other struct, so long as the fields are named (you
/// can't use an unnamed field struct). Any types you add must also implement
/// [`Send`], [`Sync`], and [`Clone`].
///
/// When you specify fields in a struct, it will create a `new()` method which
/// takes all those fields by value for initialization. For example, this code:
///
/// ```
/// use std::sync::atomic::{AtomicUsize, Ordering};
/// use std::sync::Arc;
///
/// use genserver::make_registry;
///
/// #[make_registry{}]
/// struct MyRegistry {
///     counter: Arc<AtomicUsize>,
/// }
/// ```
///
/// Will generate a registry with a `new()` which has the following signature:
/// ```compile_fail
/// pub async fn start(counter: Arc<AtomicUsize>) -> Self {
///     // ...
/// }
/// ```
pub use genserver_codegen::make_registry;

/// Error wrapper type.
#[derive(Debug)]
pub enum Error<M, R> {
    OneshotRecvError(tokio::sync::oneshot::error::RecvError),
    MpscSendError(
        tokio::sync::mpsc::error::SendError<(M, Option<tokio::sync::oneshot::Sender<R>>)>,
    ),
    Timeout,
}

impl<M, R> From<tokio::sync::oneshot::error::RecvError> for Error<M, R> {
    fn from(err: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::OneshotRecvError(err)
    }
}

impl<M, R> From<tokio::sync::mpsc::error::SendError<(M, Option<tokio::sync::oneshot::Sender<R>>)>>
    for Error<M, R>
{
    fn from(
        err: tokio::sync::mpsc::error::SendError<(M, Option<tokio::sync::oneshot::Sender<R>>)>,
    ) -> Self {
        Self::MpscSendError(err)
    }
}

/// The `GenServer` trait lets you generate a server, by implementing the trait.
pub trait GenServer {
    /// Specifies the type of messages this server can receive.
    type Message;
    /// Specifies the response from calls.
    type Response;
    /// Specifies the name of the registry type created with [`make_registry`].
    type Registry;
    /// Specifies the call response type, which must implement
    /// [std::future::Future].
    type CallResponse<'a>: Future<Output = Self::Response>
    where
        Self: 'a;
    /// Specifies the call response type, which must implement
    /// [std::future::Future] and return `()`.
    type CastResponse<'a>: Future<Output = ()>
    where
        Self: 'a;

    /// Creates a new server, and receives a copy of the current registry.
    ///
    /// You should never need to call this method yourself, as it's called for
    /// you by the registry.
    fn new(registry: Self::Registry) -> Self;
    /// This function will be called whenever this server receives a call.
    fn handle_call(&mut self, message: Self::Message) -> Self::CallResponse<'_>;
    /// This function will be called whenever this server receives a cast.
    fn handle_cast(&mut self, message: Self::Message) -> Self::CastResponse<'_>;

    /// Reimplement this method to change the channel queue size for your
    /// server. Defaults to 1000 messages.
    fn channel_queue_size() -> usize {
        1_000
    }
}

/// Marker trait for registries created with [`make_registry`].
pub trait Registry {}
