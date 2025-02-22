use crate::one_to_one::NetworkManager;
use crate::websocket_handler;
use js_sys::Uint8Array;
use log::{debug, error, info};
use serde::de::DeserializeOwned;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_peers_protocol::one_to_one::{IceCandidate, SignalMessage};
use wasm_peers_protocol::SessionId;
use web_sys::{
    MessageEvent, RtcDataChannel, RtcDataChannelEvent, RtcPeerConnection,
    RtcPeerConnectionIceEvent, WebSocket,
};

/// also calls:
/// * set_data_channel_on_open
/// * set_data_channel_on_message
/// * set_data_channel_on_error
pub(crate) fn set_peer_connection_on_data_channel<T: DeserializeOwned>(
    peer_connection: &RtcPeerConnection,
    network_manager: NetworkManager,
    on_open_callback: impl FnMut() + Clone + 'static,
    on_message_callback: impl FnMut(T) + Clone + 'static,
) {
    let on_datachannel = Closure::wrap(Box::new(move |data_channel_event: RtcDataChannelEvent| {
        info!("received data channel");
        let data_channel = data_channel_event.channel();

        set_data_channel_on_open(&data_channel, on_open_callback.clone());
        set_data_channel_on_error(&data_channel);
        set_data_channel_on_message(&data_channel, on_message_callback.clone());

        network_manager.inner.borrow_mut().data_channel = Some(data_channel);
    }) as Box<dyn FnMut(RtcDataChannelEvent)>);
    peer_connection.set_ondatachannel(Some(on_datachannel.as_ref().unchecked_ref()));
    on_datachannel.forget();
}

/// handle message sent by signaling server
pub(crate) fn set_websocket_on_message(websocket: &WebSocket, peer_connection: RtcPeerConnection) {
    let websocket_clone = websocket.clone();
    let onmessage_callback = Closure::wrap(Box::new(move |ev: MessageEvent| {
        let message = match ev.data().dyn_into::<Uint8Array>() {
            Ok(message) => message.to_vec(),

            Err(_) => {
                error!("failed to deserialize onmessage callback content.");
                return;
            }
        };
        let message = match rmp_serde::from_slice(&message) {
            Ok(message) => message,
            Err(_) => {
                error!("failed to deserialize onmessage callback content.");
                return;
            }
        };
        let websocket_clone = websocket_clone.clone();
        let peer_connection_clone = peer_connection.clone();
        wasm_bindgen_futures::spawn_local(async move {
            websocket_handler::handle_websocket_message(
                message,
                peer_connection_clone,
                websocket_clone,
            )
            .await
            .unwrap_or_else(|error| {
                error!("error handling websocket message: {:?}", error);
            })
        });
    }) as Box<dyn FnMut(MessageEvent)>);
    websocket.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget();
}

/// once websocket is open, send a request to start or join a session
pub(crate) fn set_websocket_on_open(websocket: &WebSocket, session_id: SessionId) {
    let websocket_clone = websocket.clone();
    let onopen_callback = Closure::wrap(Box::new(move |_| {
        let signal_message = SignalMessage::SessionJoin(session_id);
        let signal_message =
            rmp_serde::to_vec(&signal_message).expect("failed serializing SignalMessage");
        websocket_clone
            .send_with_u8_array(&signal_message)
            .expect("failed sending start-or-join message to the websocket");
    }) as Box<dyn FnMut(JsValue)>);
    websocket.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();
}

pub(crate) fn set_peer_connection_on_negotiation_needed(peer_connection: &RtcPeerConnection) {
    let on_negotiation_needed = Closure::wrap(Box::new(move || {
        debug!("on negotiation needed event occurred");
    }) as Box<dyn FnMut()>);
    peer_connection.set_onnegotiationneeded(Some(on_negotiation_needed.as_ref().unchecked_ref()));
    on_negotiation_needed.forget();
}

pub(crate) fn set_peer_connection_on_ice_gathering_state_change(
    peer_connection: &RtcPeerConnection,
) {
    let peer_connection_clone = peer_connection.clone();
    let on_ice_gathering_state_change = Closure::wrap(Box::new(move || {
        debug!(
            "ice gathering state: {:?}",
            peer_connection_clone.ice_gathering_state()
        );
    }) as Box<dyn FnMut()>);
    peer_connection.set_onicegatheringstatechange(Some(
        on_ice_gathering_state_change.as_ref().unchecked_ref(),
    ));
    on_ice_gathering_state_change.forget();
}

pub(crate) fn set_data_channel_on_message<T: DeserializeOwned>(
    data_channel: &RtcDataChannel,
    mut on_message_callback: impl FnMut(T) + 'static,
) {
    let datachannel_on_message = Closure::wrap(Box::new(move |ev: MessageEvent| {
        let message = ev.data().dyn_into::<Uint8Array>().ok();
        if let Some(message) = message.and_then(|t| rmp_serde::from_slice(&t.to_vec()).ok()) {
            debug!("message from datachannel (will call on_message)");
            on_message_callback(message);
        }
    }) as Box<dyn FnMut(MessageEvent)>);
    data_channel.set_onmessage(Some(datachannel_on_message.as_ref().unchecked_ref()));
    datachannel_on_message.forget();
}

pub(crate) fn set_data_channel_on_error(data_channel: &RtcDataChannel) {
    let onerror = Closure::wrap(Box::new(move |data_channel_error| {
        error!("data channel error: {:?}", data_channel_error);
    }) as Box<dyn FnMut(JsValue)>);
    data_channel.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();
}

pub(crate) fn set_data_channel_on_open(
    data_channel: &RtcDataChannel,
    mut on_open_callback: impl FnMut() + 'static,
) {
    let onopen_callback = Closure::wrap(Box::new(move |_| {
        debug!("data channel is now open, calling on_open!");
        on_open_callback();
    }) as Box<dyn FnMut(JsValue)>);
    data_channel.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
    onopen_callback.forget();
}

pub(crate) fn set_peer_connection_on_ice_connection_state_change(
    peer_connection: &RtcPeerConnection,
) {
    let peer_connection_clone = peer_connection.clone();
    let on_ice_connection_state_change = Closure::wrap(Box::new(move || {
        debug!(
            "connection state change: {:?}",
            peer_connection_clone.ice_connection_state()
        )
    }) as Box<dyn FnMut()>);
    peer_connection.set_oniceconnectionstatechange(Some(
        on_ice_connection_state_change.as_ref().unchecked_ref(),
    ));
    on_ice_connection_state_change.forget();
}

pub(crate) fn set_peer_connection_on_ice_candidate(
    peer_connection: &RtcPeerConnection,
    websocket: WebSocket,
    session_id: SessionId,
) {
    let on_ice_candidate = Closure::wrap(Box::new(move |ev: RtcPeerConnectionIceEvent| {
        let candidate = if let Some(candidate) = ev.candidate() {
            candidate
        } else {
            return;
        };
        let signaled_candidate = IceCandidate {
            candidate: candidate.candidate(),
            sdp_mid: candidate.sdp_mid(),
            sdp_m_line_index: candidate.sdp_m_line_index(),
        };
        debug!("signaled candidate: {:#?}", signaled_candidate);

        let signal_message = SignalMessage::IceCandidate(session_id, signaled_candidate);
        let signal_message =
            rmp_serde::to_vec(&signal_message).expect("failed to serialize SignalMessage");

        websocket
            .send_with_u8_array(&signal_message)
            .unwrap_or_else(|_| error!("failed to send one of the ICE candidates"));
    }) as Box<dyn FnMut(RtcPeerConnectionIceEvent)>);
    peer_connection.set_onicecandidate(Some(on_ice_candidate.as_ref().unchecked_ref()));
    on_ice_candidate.forget();
}
