// #[sides(client#not-selfhost)]

use reqwest::{cookie::{Jar, CookieStore}, Client, Response};
use tokio_tungstenite::{connect_async, WebSocketStream};
use tungstenite::ClientRequestBuilder;
use postcard::{to_stdvec, from_bytes};
use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use totp_rs::{Algorithm, TOTP};
use tungstenite::Message;
use std::sync::Arc;

use crate::crosstyping::*;


type MayTls = tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>;
pub struct RemoteDatabase {
    up: mpsc::UnboundedSender<ServerboundUpdate>,
    down: mpsc::UnboundedReceiver<ClientboundUpdate>,
    init_data: Option<(CachedStats, CachedStats, Vec<Expense>)>,
}
impl RemoteDatabase {
    async fn login(api_base: &str, device: &str, secret: Vec<u8>) -> (Response, Arc<Jar>) {
        let totp = TOTP::new(Algorithm::SHA1, 8, 1, 20, secret).unwrap();
        let code = totp.generate_current().unwrap();
        
        let jar = Arc::new(Jar::default());
        let path = api_base.to_owned() + "/api/login/" + device;
        let client = Client::builder().cookie_provider(jar.clone()).build().unwrap();
        let response = client.post(path).body(code).send().await.unwrap();
        (response.error_for_status().unwrap(), jar)
    }
    
    async fn serve(mut conn: WebSocketStream<MayTls>) -> Self {
        let (up, mut up_rx) = mpsc::unbounded_channel();
        let (down_tx, down) = mpsc::unbounded_channel();
        let (init_data_tx, init_data) = oneshot::channel();
        let mut init_data_tx = Some(init_data_tx);
        
        tokio::task::spawn(async move {
            loop {tokio::select!{
                msg_result = conn.next() => {
                    let Some(Ok(msg)) = msg_result else {return};
                    let Message::Binary(m) = msg else {continue};
                    let Ok(inbound): Result<ClientboundUpdate, _> = from_bytes(&m) else {return};
                    
                    match inbound {
                        ClientboundUpdate::InitStats{mut lifetime_stats, recent_expenses} => {
                            let Some(init_data_tx) = init_data_tx.take() else {continue};
                            
                            // we will calculate stats on this thread, not on GUI one
                            lifetime_stats.set_indices();
                            let mut month_stats = CachedStats::default();
                            recent_expenses.iter().for_each(|e| month_stats.add(e));
                            let _ = init_data_tx.send((lifetime_stats, month_stats, recent_expenses));
                        },
                        i => {
                            if let Err(_) = down_tx.send(i) {return;}
                        }
                    }
                },
                up_query = up_rx.recv() => {
                    let Some(up_query) = up_query else {return};
                    let b = Message::Binary(to_stdvec(&up_query).unwrap().into());
                    if let Err(_) = conn.send(b).await {return;}
                }
            }}
        });
        let init_data = init_data.await.ok();
        
        Self {up, down, init_data}
    }
    
    pub async fn connect(api_base: &str, credential: (&str, Vec<u8>)) -> Self {
        let path = api_base.to_owned() + "/ws";
        let rq_path = path.parse().unwrap();
        let tt_path = path.replacen("http", "ws", 1).parse().unwrap();
        
        let (_, auth_response) = Self::login(api_base, credential.0, credential.1).await;
        let cookie = auth_response.cookies(&rq_path).expect("need cookie");
        let cookie_str = cookie.to_str().unwrap();
        let builder = ClientRequestBuilder::new(tt_path).with_header("Cookie", cookie_str);
        let (conn, _response) = connect_async(builder).await.unwrap();
        
        Self::serve(conn).await
    }
}
impl Upstream for RemoteDatabase {
    fn submit(&mut self, d: ServerboundUpdate) {
        self.up.send(d).unwrap();
    }
    fn sync(&mut self) -> Vec<ClientboundUpdate> {
        match self.down.try_recv().ok() {
            Some(v) => vec![v],
            None    => vec![]
        }
    }
    fn take_init(&mut self) -> Option<(CachedStats, CachedStats, Vec<Expense>)> {
        self.init_data.take()
    }
}

