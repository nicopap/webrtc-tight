use js_sys::{Array, Object, Reflect};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{RtcConfiguration, RtcPeerConnection};
use web_sys::{RtcSdpType, RtcSessionDescriptionInit};

/// Specifies what kind of peer connection to create
#[derive(Debug, Clone)]
pub enum ConnectionType {
    /// Within local network
    Local,
    /// Setup with STUN server, WAN capabilities but can fail
    Stun { host: String },
    /// Setup with STUN and TURN hosts and fallback to TURN if needed, most stable connection
    StunAndTurn {
        host: String,
        username: String,
        credential: String,
    },
}
impl ConnectionType {
    pub(crate) fn create_peer_connection(&self) -> Result<RtcPeerConnection, JsValue> {
        use ConnectionType::{Local, Stun, StunAndTurn};
        match self {
            Local => RtcPeerConnection::new(),
            Stun { host } => {
                let ice_servers = Array::new();
                let server_entry = Object::new();

                // NOTE: it's plural, but also accepts unique string
                let url = "stun:".to_owned() + host;
                Reflect::set(&server_entry, &"urls".into(), &url.into())?;

                ice_servers.push(&*server_entry);

                let mut rtc_configuration = RtcConfiguration::new();
                rtc_configuration.ice_servers(&ice_servers);

                RtcPeerConnection::new_with_configuration(&rtc_configuration)
            }
            StunAndTurn {
                host,
                username,
                credential,
            } => {
                let ice_servers = Array::new();
                let stun_server_entry = Object::new();

                // NOTE: it's plural, but also accepts unique string
                let url = "stun:".to_owned() + host;
                Reflect::set(&stun_server_entry, &"urls".into(), &url.into())?;

                ice_servers.push(&*stun_server_entry);
                let turn_server_entry = Object::new();

                let url = "turn:".to_owned() + host;
                Reflect::set(&turn_server_entry, &"urls".into(), &url.into())?;
                Reflect::set(&turn_server_entry, &"username".into(), &username.into())?;
                Reflect::set(&turn_server_entry, &"credential".into(), &credential.into())?;

                ice_servers.push(&*turn_server_entry);

                let mut rtc_configuration = RtcConfiguration::new();
                rtc_configuration.ice_servers(&ice_servers);

                RtcPeerConnection::new_with_configuration(&rtc_configuration)
            }
        }
    }
}

pub(crate) async fn create_sdp_offer(
    peer_connection: &RtcPeerConnection,
) -> Result<String, JsValue> {
    let offer = JsFuture::from(peer_connection.create_offer())
        .await
        .map_err(|error| {
            JsValue::from_str(&format!(
                "failed to create an SDP offer: {}",
                error.as_string().unwrap_or_default()
            ))
        })?;
    let offer = Reflect::get(&offer, &JsValue::from_str("sdp"))?
        .as_string()
        .expect("failed to create JS object for SDP offer");
    let mut local_session_description = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    local_session_description.sdp(&offer);
    JsFuture::from(peer_connection.set_local_description(&local_session_description))
        .await
        .map_err(|error| {
            JsValue::from_str(&format!(
                "failed to set local description: {}",
                error.as_string().unwrap_or_default()
            ))
        })?;

    Ok(offer)
}

pub(crate) async fn create_sdp_answer(
    peer_connection: &RtcPeerConnection,
    offer: String,
) -> Result<String, JsValue> {
    let mut remote_session_description = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
    remote_session_description.sdp(&offer);
    JsFuture::from(peer_connection.set_remote_description(&remote_session_description)).await?;

    let answer = JsFuture::from(peer_connection.create_answer()).await?;
    let answer = Reflect::get(&answer, &JsValue::from_str("sdp"))?
        .as_string()
        .expect("failed to create JS object for SPD answer");

    let mut local_session_description = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
    local_session_description.sdp(&answer);
    JsFuture::from(peer_connection.set_local_description(&local_session_description)).await?;

    Ok(answer)
}

#[cfg(test)]
mod test {
    use super::*;
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
    use web_sys::{RtcIceConnectionState, RtcIceGatheringState};

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_create_stun_peer_connection_is_successful() {
        let peer_connection = ConnectionType::Local
            .create_peer_connection()
            .expect("creating peer connection failed!");
        assert_eq!(
            peer_connection.ice_connection_state(),
            RtcIceConnectionState::New
        );
        assert_eq!(
            peer_connection.ice_gathering_state(),
            RtcIceGatheringState::New
        );
    }

    #[wasm_bindgen_test]
    async fn test_create_sdp_offer_is_successful() {
        let peer_connection = RtcPeerConnection::new().expect("failed to create peer connection");
        let _offer = create_sdp_offer(&peer_connection).await.unwrap();
        assert!(peer_connection.local_description().is_some());
    }

    #[wasm_bindgen_test]
    async fn test_create_sdp_answer_is_successful() {
        let peer_connection = RtcPeerConnection::new().expect("failed to create peer connection");
        let offer = create_sdp_offer(&peer_connection).await.unwrap();
        let _answer = create_sdp_answer(&peer_connection, offer).await.unwrap();
        assert!(peer_connection.local_description().is_some());
        assert!(peer_connection.remote_description().is_some());
    }
}
