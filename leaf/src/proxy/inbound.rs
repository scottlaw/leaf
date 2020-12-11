use std::io;
use std::io::Result;
use std::sync::Arc;

use async_trait::async_trait;
use log::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::stream::StreamExt;

use crate::app::dispatcher::Dispatcher;
use crate::session::{InboundSession, Session};

use super::InboundHandler;
use super::ProxyStream;
use super::SimpleStream;
use super::TcpInboundHandler;

pub struct Handler {
    tag: String,
    dispatcher: Arc<Dispatcher>,
    tcp_handler: Arc<dyn TcpInboundHandler>,
    // udp_handler: Option<Box<dyn UdpInboundHandler>,
}

impl Handler {
    pub fn new(tag: String, dispatcher: Arc<Dispatcher>, tcp: Arc<dyn TcpInboundHandler>) -> Self {
        Handler {
            tag,
            dispatcher,
            tcp_handler: tcp,
        }
    }
}

impl InboundHandler for Handler {}

#[async_trait]
impl TcpInboundHandler for Handler {
    async fn handle_inbound_tcp<'a>(
        &'a self,
        sess: Option<(Box<dyn ProxyStream>, InboundSession)>,
    ) -> Result<(Box<dyn ProxyStream>, InboundSession)> {
        let mut listener = TcpListener::bind(format!("{}:{}", "127.0.0.1", 1086).as_str())
            .await
            .unwrap();
        info!("socks inbound listening tcp {}:{}", "127.0.0.1", 1086);
        while let Some(stream) = listener.next().await {
            if let Ok(mut stream) = stream {
                let h = self.tcp_handler.clone();
                let dispatcher = self.dispatcher.clone();
                tokio::spawn(async move {
                    let source = stream
                        .peer_addr()
                        .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap());
                    let sess = InboundSession {
                        source: Some(source),
                        destination: None,
                    };
                    let (stream, sess) = h
                        .handle_inbound_tcp(Some((Box::new(SimpleStream(stream)), sess)))
                        .await
                        .unwrap();
                    let mut sess = Session {
                        source: sess.source.unwrap(),
                        destination: sess.destination.unwrap(),
                    };
                    let _ = dispatcher.dispatch_tcp(&mut sess, stream).await;
                });
            }
        }
        Err(io::Error::new(io::ErrorKind::Other, "exit"))
    }
}
