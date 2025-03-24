// #[sides(client,server)]

#[cfg(feature = "graphics")] mod db_client_view;
#[cfg(feature = "graphics")] mod graphics;
#[cfg(feature = "graphics")] mod ecs;
#[cfg(feature = "graphics")] mod pie;
#[cfg(feature = "server")] mod serv2;
mod crosstyping;
mod dbs2;
mod dbs;



#[cfg(all(feature = "graphics", not(feature = "server")))]
fn main() {
    let db = dbs::SingleUserSqlite::default();
    graphics::run_app(db);
}

#[cfg(all(feature = "graphics", feature = "server"))]
#[tokio::main]
async fn main() {
    tokio::task::spawn(serv2::serve_forever("0.0.0.0:4341"));
    let db: dbs::SingleUserSqlite = todo!();
    graphics::run_app(db);
}

#[cfg(all(not(feature = "graphics"), feature = "server"))]
#[tokio::main]
async fn main() {
    println!("Will serve on 0.0.0.0:4341.");
    serv2::serve_forever("0.0.0.0:4341").await;
}

