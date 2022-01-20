#![feature(type_alias_impl_trait)]

use warp::Filter;

mod database;
mod handlers;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let data_dir = std::env::var("DATABASE_URL")?;
    
    let pool = database::connect(data_dir.as_str()).await;
    
    let mut content_dir = data_dir.clone();
    content_dir.push_str("/content");
    
    let route = warp::path("static")
        .and(warp::fs::dir(content_dir))
        .or(handlers::filters(pool));

    warp::serve(route).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}
