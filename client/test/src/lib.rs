pub mod common;
pub use common::xdebug;
pub use common::xtime;

pub mod fps_counter;
use wasm_bindgen::prelude::*;

use rand::prelude::*;

//use rand::{thread_rng, Rng};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn start(){

    let mut rng = rand::thread_rng();
    let s = rng.gen_range(-15.0..15.0);
    log!("lib start: {}",s);
}
