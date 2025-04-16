// #[sides(client#not-selfhost)]

use reqwest::{cookie::{Jar, Cookie, CookieStore}, Client, header::HeaderValue, Response};
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
    init_data: oneshot::Receiver<(CachedStats, CachedStats, Vec<Expense>)>,
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
    
    fn serve(mut conn: WebSocketStream<MayTls>) -> Self {
        let (up, mut up_rx) = mpsc::unbounded_channel();
        let (down_tx, down) = mpsc::unbounded_channel();
        let (init_data_tx, init_data) = oneshot::channel();
        let init_data_tx = Some(init_data_tx);
        
        tokio::task::spawn(async move {
            loop {tokio::select!{
                msg_result = conn.next() => {
                    let Some(Ok(msg)) = msg_result else {return};
                    let Message::Binary(m) = msg else {continue};
                    
                },
                up_query = up_rx.recv() => {
                    let Some(up_query) = up_query else {return};
                    let b = Message::Binary(to_stdvec(&up_query).unwrap().into());
                    if let Err(_) = conn.send(b).await {return;}
                }
            }}
        });
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
        let (conn, response) = connect_async(builder).await.unwrap();
        println!("CONNECTED {conn:?}");
        dbg!(response);
        
        Self::serve(conn)
    }
}
impl Upstream for RemoteDatabase {
    fn submit(&mut self, d: ServerboundUpdate) {
        self.up.send(d).unwrap();
    }
    fn sync(&mut self) -> Vec<ClientboundUpdate> {
        let mut buffer = Vec::new();
        self.down.blocking_recv_many(&mut buffer, 32);
        buffer
    }
    fn take_init(&mut self) -> Option<(CachedStats, CachedStats, Vec<Expense>)> {
        self.init_data.try_recv().ok()
    }
}

