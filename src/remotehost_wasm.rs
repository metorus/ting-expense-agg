// #[sides(client-browser[wasm])]

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{console, CloseEvent, Event, WebSocket};
use web_sys::js_sys::{ArrayBuffer, Uint8Array};
use postcard::{to_stdvec, from_bytes};
use futures::{channel::mpsc, StreamExt, SinkExt};
use std::sync::{Arc, Mutex};

use crate::crosstyping::*;


fn error_str() {
    
}


pub struct RemoteDatabase {
    up: WebSocket,
    down: mpsc::UnboundedReceiver<ClientboundUpdate>,
    init_data: Option<(CachedStats, CachedStats, Vec<Expense>)>,
}

impl RemoteDatabase {
    async fn serve(ws: WebSocket) -> Self {
        let up = ws.clone();
        let (down_tx, down_rx) = mpsc::unbounded();
        let (init_data_tx, init_data_rx) = futures::channel::oneshot::channel();
        let init_data_tx = Arc::new(Mutex::new(Some(init_data_tx)));
        
        // Set up message handler
        let down_tx_clone = down_tx.clone();
        let init_tx_clone = init_data_tx.clone();
        let onmessage_callback = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            // Handle binary message
            if let Ok(abuf) = e.data().dyn_into::<ArrayBuffer>() {
                let array = Uint8Array::new(&abuf);
                let mut buf = vec![0; array.length() as usize];
                array.copy_to(&mut buf);
                
                if let Ok(inbound) = from_bytes::<ClientboundUpdate>(&buf) {
                    match inbound {
                        ClientboundUpdate::InitStats{mut lifetime_stats, recent_expenses} => {
                            let mut init_tx_opt = init_tx_clone.lock().unwrap();
                            if let Some(init_tx) = init_tx_opt.take() {
                                // Calculate stats on this thread, not on GUI one
                                lifetime_stats.set_indices();
                                let mut month_stats = CachedStats::default();
                                recent_expenses.iter().for_each(|e| month_stats.add(e));
                                let _ = init_tx.send((lifetime_stats, month_stats, recent_expenses));
                            }
                        },
                        i => {
                            let _ = down_tx_clone.unbounded_send(i);
                        }
                    }
                }
            }
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();
        
        let onerror_callback = Closure::wrap(Box::new(move |e: Event| {
            console::error_2(&JsValue::from_str("WebSocket error: unknown"),
                             &e.into());
        }));
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();
        
        let onclose_callback = Closure::wrap(Box::new(move |e: CloseEvent| {
            console::error_2(&JsValue::from_str("WebSocket error: connection closed"),
                             &e.into());
        }) as Box<dyn FnMut(web_sys::CloseEvent)>);
        ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
        onclose_callback.forget();
        
        // Wait for initialization data
        let init_data = match init_data_rx.await {
            Ok(data) => Some(data),
            Err(_) => None,
        };
        
        Self { up, down: down_rx, init_data }
    }
    
    pub async fn connect(api_base: &str, _credential: ()) -> Self {
        // We assume we are logged in already.
        
        let ws_url = api_base.replacen("http", "ws", 1) + "/ws";
        let ws = WebSocket::new(&ws_url).unwrap();
        
        let (ready_tx, ready_rx) = futures::channel::oneshot::channel::<()>();
        let onopen_callback = Closure::once_into_js(move |_: web_sys::Event| {
            // An error would mean that Self::connect is cancelled, like if exitting application.
            // Also, this is an event listener, there is nothing else to do if notification fails.
            let _ = ready_tx.send(());
        });
        ws.set_onopen(Some(onopen_callback.unchecked_ref()));
        let _ = ready_rx.await;
        
        Self::serve(ws).await
    }
}

impl Upstream for RemoteDatabase {
    fn submit(&mut self, d: ServerboundUpdate) {
        if let Ok(binary_data) = to_stdvec(&d) {
            if let Err(_e) = self.up.send_with_u8_array(&binary_data) {
                
            }
        }
    }
    
    fn sync(&mut self) -> Vec<ClientboundUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.down.try_next() {
            if let Some(update) = update {
                updates.push(update);
            } else {
                break;
            }
        }
        updates
    }
    
    fn take_init(&mut self) -> Option<(CachedStats, CachedStats, Vec<Expense>)> {
        self.init_data.take()
    }
}

