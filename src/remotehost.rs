// #[sides(client#not-selfhost)]

use reqwest::{cookie::{Jar, Cookie, CookieStore}, Client, header::HeaderValue, Response};
use tokio_tungstenite::{connect_async, WebSocketStream};
use tungstenite::ClientRequestBuilder;
use totp_rs::{Algorithm, TOTP};
use std::sync::Arc;

use crate::crosstyping::*;


pub struct RemoteDatabase {
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
    
    pub async fn connect(api_base: &str, credential: (&str, Vec<u8>)) -> Self {
        let path = api_base.to_owned() + "/ws";
        let rq_path = path.parse().unwrap();
        let tt_path = path.replacen("http", "ws", 1).parse().unwrap();
        
        let (_, auth_response) = Self::login(api_base, credential.0, credential.1).await;
        let cookie = auth_response.cookies(&rq_path).expect("need cookie");
        let cookie_str = cookie.to_str().unwrap();
        let builder = ClientRequestBuilder::new(tt_path).with_header("Cookie", cookie_str);
        let conn = connect_async(builder).await.unwrap();
        println!("CONNECTED {conn:?}");
        
        unimplemented!()
    }
}
impl Upstream for RemoteDatabase {
    fn submit(&mut self, _d: ServerboundUpdate) {
        todo!()
    }
    fn sync(&mut self) -> Vec<ClientboundUpdate> {
        todo!()
    }
    fn take_init(&mut self) -> Option<(CachedStats, CachedStats, Vec<Expense>)> {
        Some(Default::default())
    }
}

