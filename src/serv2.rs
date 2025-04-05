use axum::extract::{State, Extension, Path, Request, ws::{CloseFrame, Message, close_code}};
use axum_extra::extract::{cookie::{Key, Cookie, SameSite}, SignedCookieJar};
use axum::{extract::WebSocketUpgrade, response::IntoResponse};
use axum::{routing::{get, post}, Router, RequestExt};
use axum::middleware::map_request_with_state;
use tokio::sync::broadcast::error::RecvError;
use postcard::{to_stdvec, from_bytes};
use tokio::sync::oneshot::Sender;
use axum::response::Redirect;
use tokio::net::TcpListener;
use axum::http::HeaderMap;
use futures::*;

use std::sync::Arc;

use crate::crosstyping::DownstreamMessage;
use crate::dbs2::MultiuserDb;


#[derive(Clone)]
pub struct UserAuth(String);


fn logon_cookie(principal: String) -> Cookie<'static> {
    let mut login_cookie = Cookie::new("user", principal);
    login_cookie.set_path("/");
    login_cookie.set_same_site(Some(SameSite::Strict));
    login_cookie.set_http_only(true);
    login_cookie
}


pub async fn login(
    State(db): State<Arc<MultiuserDb>>,
    Extension(jar): Extension<SignedCookieJar>,
    Path(device): Path<String>,
    totp: String
) -> impl IntoResponse {
    let principal = db.login_impl(&device, &totp).await.map_err(|e| e.to_string())?;
    Ok::<_, String>((jar.add(logon_cookie(principal)), Redirect::to("/api/me")))
}
pub async fn register(
    State(db): State<Arc<MultiuserDb>>,
    Extension(jar): Extension<SignedCookieJar>,
    maybe_auth: Option<Extension<UserAuth>>,
    Path(device): Path<String>,
    principal_reg: Option<String>
) -> impl IntoResponse {
    let (totp, principal) = match (maybe_auth, principal_reg) {
        (Some(Extension(UserAuth(principal))), None) => {
            (db.register_from(&principal, &device).await.map_err(|e| e.to_string())?, principal)
        },
        (None, Some(principal)) => {
            (db.register_impl(&device, &principal).await.map_err(|e| e.to_string())?, principal)
        },
        _ => return Err("cannot register in name of other principal when logged in".to_owned()),
    };
    Ok((jar.add(logon_cookie(principal)), totp))
}
pub async fn handle_me(
    maybe_principal: Option<Extension<UserAuth>>,
) -> String {
    maybe_principal.map(|e| e.0.0).unwrap_or(String::new())
}


pub async fn handle_websocket(
    State(db): State<Arc<MultiuserDb>>,
    Extension(UserAuth(principal)): Extension<UserAuth>,
    ws: WebSocketUpgrade
) -> impl IntoResponse {
    ws.on_upgrade(|sock| async move {
        let mut update_receiver = db.subscribe(principal.clone()).await;
        db.load(&principal).await.unwrap();
        let (mut ws_write, mut ws_read) = sock.split();
        
        // Cancellation safety is not documented so we must handle it ourselves.
        let mut ws_read_future = ws_read.next().boxed();
        
        let close_code = loop {tokio::select! {
            // Cancel-safe.
            // https://docs.rs/tokio/1.43.0/tokio/sync/broadcast/struct.Receiver.html#cancel-safety
            clientbound = update_receiver.recv() => {
                match clientbound {
                    // Forwarding message to the connected client.
                    Ok(upstream_msg) => {
                        let bytes_msg = match to_stdvec(&upstream_msg) {
                            Ok(b) => b,
                            Err(e) => {
                                eprintln!("postcard ser failed: {e:?}");
                                break close_code::ERROR
                            }
                        };
                        let msg = Message::Binary(bytes_msg);
                        if let Err(e) = ws_write.send(msg).await {
                            eprintln!("WS sending failed: {e:?}");
                            break close_code::ERROR
                        }
                    },
                    
                    // It is not generally possible for Sender to be lost
                    // since we hold Arc<MultiuserDb>, but appropriate WS
                    // status code is still AWAY (1001, server shutdown).
                    Err(RecvError::Closed) => {
                        eprintln!("unreachable: clientbound channel lost");
                        break close_code::AWAY
                    },
                    
                    // If we lagged on receiving any messages, invariants
                    // for the client would be broken by forwarding next
                    // messages out of order. Instructing the client to
                    // reconnect.
                    Err(RecvError::Lagged(_)) => break close_code::AGAIN,
                }
            },
            
            // Polling a future by mutable reference never cancels it.
            serverbound = &mut ws_read_future => {
                let Some(Ok(serverbound)) = serverbound else {
                    break close_code::NORMAL
                };
                
                let bytes_msg = match serverbound {
                    Message::Close(Some(frame)) => break frame.code,
                    Message::Close(None)        => break close_code::NORMAL,
                    Message::Binary(b)          => b,
                    _ => break close_code::UNSUPPORTED,
                };
                let Ok(serverbound_req) = from_bytes(&bytes_msg) else {
                    break close_code::INVALID
                };
                
                if let Err(e) = match serverbound_req {
                    DownstreamMessage::MadeExpense{info, temp_alias} =>
                      db.submit_expense(&principal, info, temp_alias).await,
                    DownstreamMessage::Revoked{expense_id} =>
                      db.submit_revoke(&principal, expense_id).await
                } {
                    eprintln!("Database operation failed: {e:?}");
                    break close_code::ERROR
                };
                
                std::mem::drop(ws_read_future);
                ws_read_future = ws_read.next().boxed();
            }
        }};
        
        let close_frame = CloseFrame {
            code: close_code,
            reason: "".into()
        };
        let close_fut = ws_write.send(Message::Close(Some(close_frame)));
        let _ = close_fut.await;
    })
}


pub async fn serve_forever(bind_ip: &'static str, session_signing_key: Vec<u8>,
        root_key_out: Option<Sender<(&'static str, Vec<u8>)>>) {
    let db = Arc::new(MultiuserDb::mem_new());
    let session_signing_key = Key::from(&session_signing_key);
    
    if let Some(sender) = root_key_out {
        let root_totp = db.register_impl("root", "root").await.expect("root registration fault");
        let _ = sender.send(("root", root_totp));
    }
    
    let app = Router::new()
        .route("/api/register", post(register))
        .route("/api/login", post(login))
        .route("/api/me", get(handle_me))
        .route("/ws", get(handle_websocket))
        .with_state(db)
        .layer(map_request_with_state(session_signing_key,
            |State(key): State<Key>, mut request: Request<_>| async {
                let Ok(headers) = request.extract_parts::<HeaderMap>().await;
                let jar = SignedCookieJar::from_headers(&headers, key);
                
                if let Some(c) = jar.get("user") {
                    request.extensions_mut().insert(UserAuth(c.value().to_owned()));
                }
                request.extensions_mut().insert(jar);
                request
            }))
        .route("/", get("Hello, World!"));

    let listener = TcpListener::bind(bind_ip).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

