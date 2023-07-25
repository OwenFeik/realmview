#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
#![feature(drain_filter)]
#![feature(let_chains)]
#![feature(trait_alias)]
#![feature(int_roundings)]
#![feature(const_trait_impl)]
#![feature(const_slice_index)]

pub use scene;

mod bridge;
mod client;
mod dom;
mod interactor;
mod render;
mod start;
mod viewport;
