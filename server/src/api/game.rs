use actix_web::web;

use super::{res_failure, Res};

pub fn routes() -> actix_web::Scope {
    web::scope("/game")
        .route("/new", web::post().to(new))
        .route("/{game_key}", web::post().to(join))
        .route("/{game_key}/{client_key}", web::get().to(connect))
}

async fn new() -> Res {
    res_failure("no")
}

async fn join() -> Res {
    res_failure("no")
}

async fn connect() -> Res {
    res_failure("no")
}
