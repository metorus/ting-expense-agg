#[cfg(all(feature = "selfhost", feature = "server"))]
compile_error!("Select one of `selfhost`, `server` features.");

#[cfg(all(feature = "selfhost", feature = "graphics_wasm"))]
compile_error!("WASM application does not support self-hosted database (yet?)");

#[cfg(all(feature = "server", feature = "graphics_wasm"))]
compile_error!("WASM application does not provide a expense-storing server.");


#[cfg(all(feature = "graphics_wasm", not(feature = "selfhost")))] mod remotehost_wasm;
#[cfg(all(feature = "graphics_nowasm", not(feature = "selfhost")))] mod remotehost;
#[cfg(feature = "selfhost")] mod selfhost;
#[cfg(feature = "graphics")] mod db_slice;
#[cfg(feature = "graphics")] mod graphics;
#[cfg(feature = "graphics")] mod widgets;
#[cfg(feature = "server")] mod server;
mod crosstyping;

#[cfg(all(feature = "graphics_wasm", not(feature = "selfhost")))] use remotehost_wasm::RemoteDatabase;
#[cfg(all(feature = "graphics_nowasm", not(feature = "selfhost")))] use remotehost::RemoteDatabase;



#[cfg(feature = "graphics_wasm")]
fn main() {
    use wasm_bindgen_futures::spawn_local;
    
    spawn_local(async {
        let db = RemoteDatabase::connect("", ()).await;
        graphics::run_app(db);
    });
}

#[cfg(all(feature = "graphics_nowasm", feature = "selfhost"))]
fn main() {
    let db = selfhost::SingleUserSqlite::default();
    graphics::run_app(db).unwrap();
}

#[cfg(all(feature = "graphics_nowasm", not(feature = "server"), not(feature = "selfhost")))]
#[tokio::main]
async fn main() {
    let db = RemoteDatabase::connect(todo!("url"), todo!("credentials")).await;
    graphics::run_app(db);
}

#[cfg(all(feature = "graphics_nowasm", feature = "server"))]
fn main() {
    env_logger::init();
    
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (root_send, root_recv) = tokio::sync::oneshot::channel();
    runtime.spawn(server::serve_forever("0.0.0.0:4341", vec![1_u8; 64], Some(root_send)));
    let db = runtime.block_on(async {
        let root_credentials = root_recv.await.expect("TEA root account was not generated");
        RemoteDatabase::connect("http://127.0.0.1:4341", root_credentials).await
    });
    
    graphics::run_app(db).unwrap();
}

#[cfg(all(not(feature = "graphics"), feature = "server"))]
#[tokio::main]
async fn main() {
    println!("Will serve on 0.0.0.0:4341.");
    server::serve_forever("0.0.0.0:4341", vec![1_u8; 64], None).await;
}

