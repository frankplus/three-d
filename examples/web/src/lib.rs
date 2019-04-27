
use log::Level;
use wasm_bindgen::prelude::*;

include!("../../hello_world.rs");

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue>
{
    console_log::init_with_level(Level::Debug).unwrap();

    use log::info;
    info!("Logging works!");

    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    main();
    Ok(())
}
