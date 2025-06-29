#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
#![feature(let_chains)]
#![feature(trait_alias)]
#![feature(int_roundings)]
#![feature(const_trait_impl)]
#![feature(stmt_expr_attributes)]

pub use scene;

mod bridge;
mod client;
mod dom;
mod interactor;
mod render;
mod start;
mod viewport;

type Res<T> = Result<T, String>;

fn err<T, S: ToString>(s: S) -> Res<T> {
    Err(s.to_string())
}
