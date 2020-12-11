use std::io;
use std::net::IpAddr;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use log::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::stream::StreamExt;

use crate::{
    app::dispatcher::Dispatcher,
    proxy::ProxyStream,
    proxy::TcpInboundHandler,
    session::{InboundSession, Session, SocksAddr, SocksAddrWireType},
    Runner,
};

pub struct Handler {}

impl Handler {
    pub fn new() -> Self {
        Handler {}
    }
}

#[async_trait]
impl TcpInboundHandler for Handler {
    async fn handle_inbound_tcp<'a>(
        &'a self,
        sess: Option<(Box<dyn ProxyStream>, InboundSession)>,
    ) -> std::io::Result<(Box<dyn ProxyStream>, InboundSession)> {
        if let Some(sess) = sess {
            let (mut stream, sess) = sess;

            let mut buf = BytesMut::with_capacity(1024);

            // handle auth
            buf.resize(2, 0);
            // ver, nmethods
            if let Err(e) = stream.read_exact(&mut buf[..]).await {
                debug!("read ver, nmethods failed: {}", e);
                return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
            };
            if buf[0] != 0x05 {
                warn!("unknown socks version {}", buf[0]);
                return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
            }
            if buf[1] == 0 {
                warn!("no socks5 authentication method specified");
                return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
            }
            let nmethods = buf[1] as usize;
            buf.resize(nmethods, 0);
            // methods
            if let Err(e) = stream.read_exact(&mut buf[..]).await {
                debug!("read methods failed: {}", e);
                return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
            };
            let mut method_accepted = false;
            let mut method_idx: u8 = 0;
            let supported_method: u8 = 0x0;
            for (idx, method) in buf[..].iter().enumerate() {
                if method == &supported_method {
                    method_accepted = true;
                    method_idx = idx as u8;
                    break;
                }
            }
            if !method_accepted {
                warn!("unsupported socks5 authentication methods");
                if let Err(e) = stream.write_all(&[0x05, 0xff]).await {
                    debug!("write auth response failed: {}", e);
                };
                return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
            } else if let Err(e) = stream.write_all(&[0x05, method_idx]).await {
                debug!("write auth response failed: {}", e);
                return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
            };

            // handle request
            buf.resize(3, 0);
            // ver, cmd, rsv
            if let Err(e) = stream.read_exact(&mut buf[..]).await {
                debug!("read request failed: {}", e);
                return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
            };
            if buf[0] != 0x05 {
                warn!("unknown socks version {}", buf[0]);
                // TODO reply?
                return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
            }
            if buf[2] != 0x0 {
                warn!("non-zero socks5 reserved field");
                // TODO reply?
                return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
            }
            let cmd = buf[1];
            match cmd {
                // connect
                0x01 => {}
                // udp associate
                0x03 => {}
                _ => {
                    warn!("unsupported socks5 cmd {}", cmd);
                    // TODO reply?
                    return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
                }
            }
            let destination =
                match SocksAddr::read_from(&mut stream, SocksAddrWireType::PortLast).await {
                    Ok(v) => v,
                    Err(e) => {
                        debug!("read address failed: {}", e);
                        return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
                    }
                };

            match cmd {
                0x01 => {
                    // handle response
                    buf.clear();
                    buf.put_u8(0x05); // version 5
                    buf.put_u8(0x0); // succeeded
                    buf.put_u8(0x0); // rsv
                    let resp_addr = SocksAddr::empty_ipv4();
                    if let Err(e) = resp_addr.write_buf(&mut buf, SocksAddrWireType::PortLast) {
                        debug!("write address buffer: {}", e);
                        return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
                    };
                    if let Err(e) = stream.write_all(&buf[..]).await {
                        debug!("write response failed: {}", e);
                        return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
                    };

                    // let source = stream
                    //     .peer_addr()
                    //     .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap());
                    let source = "0.0.0.0:0".parse().unwrap();
                    let sess = InboundSession {
                        source: Some(source),
                        destination: Some(destination),
                    };

                    return Ok((stream, sess));
                }
                0x03 => {
                    buf.clear();
                    buf.put_u8(0x05); // version 5
                    buf.put_u8(0x0); // succeeded
                    buf.put_u8(0x0); // rsv
                    let resp_addr = SocksAddr::from(("127.0.0.1", 1086));
                    if let Err(e) = resp_addr.write_buf(&mut buf, SocksAddrWireType::PortLast) {
                        debug!("write address buffer: {}", e);
                        return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
                    };
                    if let Err(e) = stream.write_all(&buf[..]).await {
                        debug!("write response failed: {}", e);
                        return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
                    };
                    let mut buf = [0u8; 1];
                    // TODO explicitly drop resources allocated above before waiting?
                    if stream.read_exact(&mut buf).await.is_err() {
                        // perhaps explicitly notifies the NAT manager?
                        debug!("udp association end");
                    }
                }
                _ => (),
            }
        }
        return Err(io::Error::new(io::ErrorKind::Other, "unspecified"));
    }
}

pub fn new(
    listen: String,
    port: u16,
    bind_addr: String,
    dispatcher: Arc<Dispatcher>,
) -> Result<Runner> {
    let t =
        async move {
            let mut listener = TcpListener::bind(format!("{}:{}", listen.clone(), port).as_str())
                .await
                .unwrap();
            info!("socks inbound listening tcp {}:{}", listen.clone(), port);
            let bind_addr = bind_addr
                .parse::<IpAddr>()
                .expect("illegal socks5 udp bind address");
            while let Some(stream) = listener.next().await {
                if let Ok(mut stream) = stream {
                    let dispatcher = dispatcher.clone();
                    let bind_addr = bind_addr;
                    tokio::spawn(async move {
                        let mut buf = BytesMut::with_capacity(1024);

                        // handle auth
                        buf.resize(2, 0);
                        // ver, nmethods
                        if let Err(e) = stream.read_exact(&mut buf[..]).await {
                            debug!("read ver, nmethods failed: {}", e);
                            return;
                        };
                        if buf[0] != 0x05 {
                            warn!("unknown socks version {}", buf[0]);
                            return;
                        }
                        if buf[1] == 0 {
                            warn!("no socks5 authentication method specified");
                            return;
                        }
                        let nmethods = buf[1] as usize;
                        buf.resize(nmethods, 0);
                        // methods
                        if let Err(e) = stream.read_exact(&mut buf[..]).await {
                            debug!("read methods failed: {}", e);
                            return;
                        };
                        let mut method_accepted = false;
                        let mut method_idx: u8 = 0;
                        let supported_method: u8 = 0x0;
                        for (idx, method) in buf[..].iter().enumerate() {
                            if method == &supported_method {
                                method_accepted = true;
                                method_idx = idx as u8;
                                break;
                            }
                        }
                        if !method_accepted {
                            warn!("unsupported socks5 authentication methods");
                            if let Err(e) = stream.write_all(&[0x05, 0xff]).await {
                                debug!("write auth response failed: {}", e);
                            };
                            return;
                        } else if let Err(e) = stream.write_all(&[0x05, method_idx]).await {
                            debug!("write auth response failed: {}", e);
                            return;
                        };

                        // handle request
                        buf.resize(3, 0);
                        // ver, cmd, rsv
                        if let Err(e) = stream.read_exact(&mut buf[..]).await {
                            debug!("read request failed: {}", e);
                            return;
                        };
                        if buf[0] != 0x05 {
                            warn!("unknown socks version {}", buf[0]);
                            // TODO reply?
                            return;
                        }
                        if buf[2] != 0x0 {
                            warn!("non-zero socks5 reserved field");
                            // TODO reply?
                            return;
                        }
                        let cmd = buf[1];
                        match cmd {
                            // connect
                            0x01 => {}
                            // udp associate
                            0x03 => {}
                            _ => {
                                warn!("unsupported socks5 cmd {}", cmd);
                                // TODO reply?
                                return;
                            }
                        }
                        let destination =
                            match SocksAddr::read_from(&mut stream, SocksAddrWireType::PortLast)
                                .await
                            {
                                Ok(v) => v,
                                Err(e) => {
                                    debug!("read address failed: {}", e);
                                    return;
                                }
                            };

                        match cmd {
                            0x01 => {
                                // handle response
                                buf.clear();
                                buf.put_u8(0x05); // version 5
                                buf.put_u8(0x0); // succeeded
                                buf.put_u8(0x0); // rsv
                                let resp_addr = SocksAddr::empty_ipv4();
                                if let Err(e) =
                                    resp_addr.write_buf(&mut buf, SocksAddrWireType::PortLast)
                                {
                                    debug!("write address buffer: {}", e);
                                    return;
                                };
                                if let Err(e) = stream.write_all(&buf[..]).await {
                                    debug!("write response failed: {}", e);
                                    return;
                                };

                                let source = stream
                                    .peer_addr()
                                    .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap());
                                let mut sess = Session {
                                    source,
                                    destination,
                                };

                                let _ = dispatcher.dispatch_tcp(&mut sess, stream).await;
                            }
                            0x03 => {
                                buf.clear();
                                buf.put_u8(0x05); // version 5
                                buf.put_u8(0x0); // succeeded
                                buf.put_u8(0x0); // rsv
                                let resp_addr = SocksAddr::from((bind_addr, port));
                                if let Err(e) =
                                    resp_addr.write_buf(&mut buf, SocksAddrWireType::PortLast)
                                {
                                    debug!("write address buffer: {}", e);
                                    return;
                                };
                                if let Err(e) = stream.write_all(&buf[..]).await {
                                    debug!("write response failed: {}", e);
                                    return;
                                };
                                let mut buf = [0u8; 1];
                                // TODO explicitly drop resources allocated above before waiting?
                                if stream.read_exact(&mut buf).await.is_err() {
                                    // perhaps explicitly notifies the NAT manager?
                                    debug!("udp association end");
                                }
                            }
                            _ => (),
                        }
                    });
                }
            }
        };

    Ok(Box::pin(t))
}
