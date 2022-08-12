#![feature(type_alias_impl_trait, generic_associated_types)]

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
