use std::collections::HashMap;
use std::sync::Arc;

use crate::app::dispatcher::Dispatcher;
use crate::proxy::InboundHandler;
use crate::Runner;

pub struct InboundManager {
    // handlers: HashMap<String, Arc<dyn InboundHandler>>,
    handlers: Vec<Arc<dyn InboundHandler>>,
}

impl InboundManager {
    pub fn new(dispatcher: Arc<Dispatcher>) -> Self {
        // let mut handlers: HashMap<String, Arc<dyn InboundHandler>> = HashMap::new();
        let mut handlers: Vec<Arc<dyn InboundHandler>> = Vec::new();
        let socks_tcp = Arc::new(crate::proxy::socks::inbound::TcpHandler::new());
        let handler = Arc::new(crate::proxy::inbound::Handler::new(
            "socks".to_string(),
            dispatcher.clone(),
            socks_tcp,
        ));
        // handlers.insert("socks".to_string(), handler);
        handlers.push(handler);
        InboundManager { handlers }
    }

    pub fn get_runners(mut self) -> Vec<Runner> {
        let mut runners: Vec<Runner> = Vec::new();
        // for (_, handler) in self.handlers.iter_mut() {
        //     let t = async move {
        //         handler.handle_inbound_tcp(None).await;
        //     };
        //     runners.push(Box::pin(t));
        // }
        for handler in self.handlers {
            let t = async move {
                handler.handle_inbound_tcp(None).await;
            };
            runners.push(Box::pin(t));
        }
        runners
    }
}
