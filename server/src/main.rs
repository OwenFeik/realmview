use warp::Filter;

#[tokio::main]
async fn main() {
    let content_dir = std::env::args().nth(1).expect("Usage: server path/to/content");

    let route = warp::path("static").and(warp::fs::dir(content_dir));

    warp::serve(route).run(([127, 0, 0, 1], 3030)).await;
}
