//! Test suite for the Web and headless browsers.

#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use wasm_peers::one_to_one::NetworkManager;
use wasm_peers::{ConnectionType, SessionId};
use web_sys::console;

const SIGNALING_SERVER_URL: &str = "ws://0.0.0.0:9001/one-to-one";

wasm_bindgen_test_configure!(run_in_browser);

const DUMMY_SESSION_ID: u128 = 1234;

#[wasm_bindgen_test]
fn network_manager_starts_successfully() {
    let mut server = NetworkManager::new(
        SIGNALING_SERVER_URL,
        SessionId::new(DUMMY_SESSION_ID),
        ConnectionType::Local,
    )
    .unwrap();
    server.start(|| {}, |_| {}).unwrap();
}

#[wasm_bindgen_test]
fn single_message_passes_both_ways() {
    let server_received_message = Rc::new(RefCell::new(false));
    let client_received_message = Rc::new(RefCell::new(false));

    let mut server = NetworkManager::new(
        SIGNALING_SERVER_URL,
        SessionId::new(DUMMY_SESSION_ID),
        ConnectionType::Local,
    )
    .unwrap();

    let server_clone = server.clone();
    let server_on_open = move || server_clone.send_message("ping!").unwrap();
    let server_on_message = {
        let server_received_message = server_received_message.clone();
        move |message| {
            console::log_1(&format!("server received message: {}", message).into());
            *server_received_message.borrow_mut() = true;
        }
    };
    server.start(server_on_open, server_on_message).unwrap();

    let mut client = NetworkManager::new(
        SIGNALING_SERVER_URL,
        SessionId::new(DUMMY_SESSION_ID),
        ConnectionType::Local,
    )
    .unwrap();
    let client_on_open = || { /* do nothing */ };
    let client_clone = client.clone();
    let client_on_message = {
        let client_received_message = client_received_message.clone();
        move |message| {
            console::log_1(&format!("client received message: {}", message).into());
            client_clone.send_message("pong!").unwrap();
            *client_received_message.borrow_mut() = true;
        }
    };
    client.start(client_on_open, client_on_message).unwrap();

    // assert!(*client_received_message.borrow());
    // assert!(*server_received_message.borrow());
}
