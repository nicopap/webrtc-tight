use wasm_bindgen::prelude::*;

#[wasm_bindgen(raw_module = "./wasm_peer_config.js")]
extern "C" {
    pub(crate) fn turn_credential() -> String;
    pub(crate) fn turn_username() -> String;
    pub(crate) fn server() -> String;
}
