#[cfg(all(feature = "selfhost", feature = "server"))]
compile_error!("Select one of `selfhost`, `server` features.");

#[cfg(all(feature = "graphics", not(feature = "selfhost")))] mod remotehost;
#[cfg(feature = "selfhost")] mod selfhost;
#[cfg(feature = "graphics")] mod db_slice;
#[cfg(feature = "graphics")] mod graphics;
#[cfg(feature = "graphics")] mod widgets;
#[cfg(feature = "server")] mod server;
mod crosstyping;



#[cfg(all(feature = "graphics", feature = "selfhost"))]
fn main() {
    let db = selfhost::SingleUserSqlite::default();
    graphics::run_app(db).unwrap();
}

#[cfg(all(feature = "graphics", not(feature = "server"), not(feature = "selfhost")))]
fn main() {
    unimplemented!()
    // let db: dbs::SingleUserSqlite = todo!();
    // graphics::run_app(db).unwrap();
}

#[cfg(all(feature = "graphics", feature = "server"))]
fn main() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    let (root_send, root_recv) = tokio::sync::oneshot::channel();
    runtime.spawn(server::serve_forever("0.0.0.0:4341", vec![1_u8; 64], Some(root_send)));
    let db = runtime.block_on(async {
        let root_credentials = root_recv.await.expect("TEA root account could not be generated");
        remotehost::RemoteDatabase::connect("http://127.0.0.1:4341", root_credentials).await
    });
    
    graphics::run_app(db).unwrap();
}

#[cfg(all(not(feature = "graphics"), feature = "server"))]
#[tokio::main]
async fn main() {
    println!("Will serve on 0.0.0.0:4341.");
    server::serve_forever("0.0.0.0:4341", vec![1_u8; 64], None).await;
}

