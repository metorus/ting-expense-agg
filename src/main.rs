// #[sides(client,server)]

#[cfg(feature = "graphics")] mod db_client_view;
#[cfg(feature = "graphics")] mod graphics;
#[cfg(feature = "graphics")] mod ecs;
#[cfg(feature = "graphics")] mod pie;
#[cfg(feature = "server")] mod serv2;
#[cfg(feature = "server")] mod dbs2;
mod crosstyping;
mod dbs;



#[cfg(all(feature = "graphics", not(feature = "server")))]
fn main() {
    let db = dbs::SingleUserSqlite::default();
    graphics::run_app(db).unwrap();
}

#[cfg(all(feature = "graphics", feature = "server"))]
#[tokio::main]
async fn main() {
    let (root_send, root_recv) = tokio::sync::oneshot::channel();
    tokio::task::spawn(serv2::serve_forever("0.0.0.0:4341", vec![1_u8; 64], Some(root_send)));
    let _root_credentials = root_recv.await.expect("TEA root account could not be generated");
    
    let db: dbs::SingleUserSqlite = todo!();
    graphics::run_app(db).unwrap();
}

#[cfg(all(not(feature = "graphics"), feature = "server"))]
#[tokio::main]
async fn main() {
    println!("Will serve on 0.0.0.0:4341.");
    serv2::serve_forever("0.0.0.0:4341", vec![1_u8; 64], None).await;
}

