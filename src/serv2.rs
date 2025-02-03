use axum::extract::{State, Extension, ws::{CloseFrame, Message, close_code}};
use axum::{extract::WebSocketUpgrade, response::IntoResponse};
use tokio::sync::broadcast::error::RecvError;
use postcard::{to_stdvec, from_bytes};
use futures::*;

use std::sync::Arc;

use crate::crosstyping::DownstreamMessage;
use crate::dbs2::MultiuserDb;


pub struct UserAuth(String);


pub async fn handle_websocket(State(db): State<Arc<MultiuserDb>>, Extension(UserAuth(principal)): Extension<UserAuth>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|sock| async move {
        let mut update_receiver = db.subscribe(principal.clone()).await;
        let (mut ws_write, mut ws_read) = sock.split();
        
        // Cancellation safety is not documented so we must handle it
        // ourselves.
        let mut ws_read_future = ws_read.next().boxed();
        
        let close_code = loop {tokio::select! {
            // Cancel-safe. https://docs.rs/tokio/1.43.0/tokio/sync/broadcast/struct.Receiver.html#cancel-safety
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

