#![feature(type_alias_impl_trait, impl_trait_in_assoc_type)]

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use genserver::{make_registry, GenServer};

#[tokio::test]
async fn test() {
    struct MyServer {
        registry: MyRegistry,
    }

    impl GenServer for MyServer {
        type Message = String;
        type Registry = MyRegistry;
        type Response = String;

        type CallResponse<'a> = impl Future<Output = Self::Response> + 'a;
        type CastResponse<'a> = impl Future<Output = ()> + 'a;

        fn new(registry: Self::Registry) -> Self {
            Self { registry }
        }

        fn handle_call(&mut self, message: Self::Message) -> Self::CallResponse<'_> {
            self.registry.counter.fetch_add(1, Ordering::SeqCst);
            std::future::ready(format!("in handle_call, received {}", message))
        }

        fn handle_cast(&mut self, message: Self::Message) -> Self::CastResponse<'_> {
            self.registry.counter.fetch_add(1, Ordering::SeqCst);
            async move {
                println!("in handle_cast, receved {}", message);
                let resp = self
                    .registry
                    .call_myserver2("calling myserver2 from myserver1".into())
                    .await
                    .unwrap();
                println!("got {} from myserver2", resp);
            }
        }
    }

    #[make_registry{
        myserver1: MyServer,
        myserver2: MyServer,
    }]
    struct MyRegistry {
        counter: Arc<AtomicUsize>,
    }

    let counter = Arc::new(AtomicUsize::new(0));
    let registry = MyRegistry::start(counter.clone()).await;

    let response = registry.call_myserver1("hi".into()).await.unwrap();
    assert_eq!("in handle_call, received hi", &response);
    registry.cast_myserver1("woohoo!".into()).await.unwrap();
    println!("got response: {}", response);

    tokio::time::sleep(Duration::from_secs(1)).await;

    assert_eq!(counter.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn test_separate_types() {
    struct MyServer {
        registry: MyRegistry,
    }

    impl GenServer for MyServer {
        type Message = String;
        type Registry = MyRegistry;
        type Response = ();

        type CallResponse<'a> = impl Future<Output = Self::Response> + 'a;
        type CastResponse<'a> = impl Future<Output = ()> + 'a;

        fn new(registry: Self::Registry) -> Self {
            Self { registry }
        }

        fn handle_call(&mut self, message: Self::Message) -> Self::CallResponse<'_> {
            println!("got {:?}", message);
            self.registry.counter.fetch_add(1, Ordering::SeqCst);
            std::future::ready(())
        }

        fn handle_cast(&mut self, message: Self::Message) -> Self::CastResponse<'_> {
            self.registry.counter.fetch_add(1, Ordering::SeqCst);
            async move {
                println!("in handle_cast, receved {}", message);
                let resp = self
                    .registry
                    .call_myserver2("calling myserver2 from myserver1".into())
                    .await
                    .unwrap();
                println!("got {:?} from myserver2", resp);
            }
        }
    }

    #[make_registry{
        myserver1: MyServer,
        myserver2: MyServer,
    }]
    struct MyRegistry {
        counter: Arc<AtomicUsize>,
    }

    let counter = Arc::new(AtomicUsize::new(0));
    let registry = MyRegistry::start(counter.clone()).await;

    let response = registry.call_myserver1("hi".into()).await.unwrap();
    assert_eq!((), response);
    registry.cast_myserver1("woohoo!".into()).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    assert_eq!(counter.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn test_timeout() {
    struct MyServer {
        registry: MyRegistry,
    }

    impl GenServer for MyServer {
        type Message = String;
        type Registry = MyRegistry;
        type Response = ();

        type CallResponse<'a> = impl Future<Output = Self::Response> + 'a;
        type CastResponse<'a> = impl Future<Output = ()> + 'a;

        fn new(registry: Self::Registry) -> Self {
            Self { registry }
        }

        fn handle_call(&mut self, message: Self::Message) -> Self::CallResponse<'_> {
            println!("got {:?}", message);
            self.registry.counter.fetch_add(1, Ordering::SeqCst);
            async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                ()
            }
        }

        fn handle_cast(&mut self, message: Self::Message) -> Self::CastResponse<'_> {
            self.registry.counter.fetch_add(1, Ordering::SeqCst);
            async move {
                println!("in handle_cast, receved {}", message);
                let resp = self
                    .registry
                    .call_myserver2("calling myserver2 from myserver1".into())
                    .await
                    .unwrap();
                println!("got {resp:?} from myserver2");
            }
        }
    }

    #[make_registry{
        myserver1: MyServer,
        myserver2: MyServer,
    }]
    struct MyRegistry {
        counter: Arc<AtomicUsize>,
    }

    let counter = Arc::new(AtomicUsize::new(0));
    let registry = MyRegistry::start(counter.clone()).await;

    let response = registry
        .call_myserver1_with_timeout("hi".into(), Duration::from_millis(1))
        .await;
    assert!(response.is_err());
    let response = registry
        .call_myserver1_with_timeout("hi".into(), Duration::from_millis(1_000))
        .await;
    assert!(response.is_ok());
    registry.cast_myserver1("woohoo!".into()).await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    assert_eq!(counter.load(Ordering::Relaxed), 4);
}

#[tokio::test]
async fn test_state_injection() {
    struct MyServer {
        registry: MyRegistry,
        initial_values: Vec<String>,
    }

    #[derive(Debug)]
    enum MyServerMessage {
        InitState(Vec<String>),
        GetState,
    }

    #[derive(Debug, PartialEq)]
    enum MyServerResponse {
        Ok,
        State(Vec<String>),
    }

    impl GenServer for MyServer {
        type Message = MyServerMessage;
        type Registry = MyRegistry;
        type Response = MyServerResponse;

        type CallResponse<'a> = impl Future<Output = Self::Response> + 'a;
        type CastResponse<'a> = impl Future<Output = ()> + 'a;

        fn new(registry: Self::Registry) -> Self {
            Self {
                registry,
                initial_values: vec![],
            }
        }

        fn handle_call(&mut self, message: Self::Message) -> Self::CallResponse<'_> {
            println!("got {message:?}");
            self.registry.counter.fetch_add(1, Ordering::SeqCst);
            async {
                match message {
                    MyServerMessage::InitState(initial_values) => {
                        self.initial_values = initial_values;
                        MyServerResponse::Ok
                    }
                    MyServerMessage::GetState => {
                        MyServerResponse::State(self.initial_values.clone())
                    }
                }
            }
        }

        fn handle_cast(&mut self, _message: Self::Message) -> Self::CastResponse<'_> {
            std::future::ready(())
        }
    }

    #[make_registry{
        myserver1: MyServer,
        myserver2: MyServer,
    }]
    struct MyRegistry {
        counter: Arc<AtomicUsize>,
    }

    let counter = Arc::new(AtomicUsize::new(0));
    let registry = MyRegistry::start(counter.clone()).await;

    let response = registry
        .call_myserver1(MyServerMessage::InitState(vec!["one".into(), "two".into()]))
        .await;
    assert!(response.is_ok());
    assert_eq!(response.unwrap(), MyServerResponse::Ok);

    let response = registry.call_myserver1(MyServerMessage::GetState).await;
    assert!(response.is_ok());
    assert_eq!(
        response.unwrap(),
        MyServerResponse::State(vec!["one".into(), "two".into()])
    );

    tokio::time::sleep(Duration::from_secs(1)).await;

    assert_eq!(counter.load(Ordering::Relaxed), 2);
}
