use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use futures::stream::{Stream, StreamExt};

use crate::error::{Result, StoreError};

/// Type for domain-specific futures.
pub struct DomainFuture<T, E> {
    receiver: oneshot::Receiver<std::result::Result<T, E>>,
    _task: JoinHandle<()>,
}

impl<T, E> DomainFuture<T, E> {
    /// Create a new DomainFuture by spawning a task that will execute the given future.
    pub fn new<F>(future: F) -> Self 
    where
        F: Future<Output = std::result::Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        
        let task = tokio::spawn(async move {
            let result = future.await;
            let _ = sender.send(result);
        });
        
        Self {
            receiver,
            _task: task,
        }
    }
}

impl<T, E> Future for DomainFuture<T, E> {
    type Output = std::result::Result<T, E>;
    
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.receiver).poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(_)) => {
                // If oneshot channel is closed, the sender was dropped - likely because 
                // the spawned task panicked or was canceled
                let error_msg = "Channel closed unexpectedly";
                eprintln!("{}", error_msg);
                panic!("{}", error_msg)
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Type for domain-specific streams.
pub struct DomainStream<T, E> {
    receiver: mpsc::Receiver<std::result::Result<T, E>>,
    _task: JoinHandle<()>,
}

impl<T, E> DomainStream<T, E> {
    /// Create a new DomainStream that wraps an async stream
    pub fn new<S, F>(stream_provider: F) -> Self 
    where
        S: Stream<Item = std::result::Result<T, E>> + Send + 'static,
        F: Future<Output = std::result::Result<S, E>> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        let (sender, receiver) = mpsc::channel(32);
        
        let task = tokio::spawn(async move {
            match stream_provider.await {
                Ok(mut stream) => {
                    while let Some(item) = stream.next().await {
                        if sender.send(item).await.is_err() {
                            break;
                        }
                    }
                }
                Err(err) => {
                    let _ = sender.send(Err(err)).await;
                }
            }
        });
        
        Self {
            receiver,
            _task: task,
        }
    }
}

/// A wrapper for futures that allows handling futures in a synchronous manner.
/// 
/// This type hides the complexity of dealing with Pin<Box<dyn Future>> and provides
/// a clean awaitable interface for the consumer.
pub struct MatrixFuture<T> {
    receiver: oneshot::Receiver<Result<T>>,
    _task: JoinHandle<()>,
}

impl<T> MatrixFuture<T> {
    /// Create a new MatrixFuture by spawning a task that will execute the given future.
    pub fn spawn<F>(future: F) -> Self 
    where
        F: Future<Output = Result<T>> + Send + 'static,
        T: Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        
        let task = tokio::spawn(async move {
            let result = future.await;
            let _ = sender.send(result);
        });
        
        Self {
            receiver,
            _task: task,
        }
    }
}

impl<T> Future for MatrixFuture<T> {
    type Output = Result<T>;
    
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.receiver).poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(_)) => Poll::Ready(Err(StoreError::StorageCommunication(
                "Channel closed unexpectedly".into(),
            ))),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// A wrapper for streams of events or data from Matrix operations.
/// 
/// This allows returning a proper Stream type without using Box<dyn Stream>.
pub struct MatrixStream<T> {
    receiver: mpsc::Receiver<Result<T>>,
    _task: JoinHandle<()>,
}

impl<T> MatrixStream<T> {
    /// Create a new MatrixStream that wraps an async stream
    pub fn spawn<S, F>(stream_provider: F) -> Self 
    where
        S: Stream<Item = Result<T>> + Send + 'static,
        F: Future<Output = Result<S>> + Send + 'static,
        T: Send + 'static,
    {
        let (sender, receiver) = mpsc::channel(32);
        
        let task = tokio::spawn(async move {
            match stream_provider.await {
                Ok(mut stream) => {
                    while let Some(item) = stream.next().await {
                        if sender.send(item).await.is_err() {
                            break;
                        }
                    }
                }
                Err(err) => {
                    let _ = sender.send(Err(err)).await;
                }
            }
        });
        
        Self {
            receiver,
            _task: task,
        }
    }
}

impl<T> Stream for MatrixStream<T> {
    type Item = Result<T>;
    
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_recv(cx)
    }
}