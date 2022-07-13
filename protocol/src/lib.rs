/*!
Helper crate that declares common types and structures shared between [wasm-peers](https://docs.rs/wasm-peers/latest/wasm_peers/)
and [wasm-peers-signaling-server](https://docs.rs/wasm-peers-signaling-server/latest/wasm_peers_signaling_server/).
*/

use serde::{Deserialize, Serialize};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

pub mod one_to_one;

/// Port used for the websocket signaling channel of the WebRTC connection.
///
/// The client will keep a connection to the server to communicate protocol-level
/// changes to its state. WebRTC is not really peer-to-peer, it requires a constant
/// connection to a third party server.
///
/// The constant connection is maintained through a websocket. This is the port used
/// for the websocket connection.
///
/// See MDN https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API/Protocols
pub const WS_PORT: u16 = 9003;

/// STUN is a server protocol to find your publicly visible IP address, it is part of
/// the WebRTC protocol.
///
/// This crate embeds a STUN server, so that you do not have any external depdendencies
/// Most online WebRTC demo depends on google's or random third party STUN server,
/// exposing your player's IP address to those nice people :), this protocol implementation
/// includes a STUN server for the security of your users and your personal GDPR compliance.
///
/// See MDN https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API/Protocols
pub const STUN_PORT: u16 = 9004;

/// TURN is an last-resort connection options going through your own server in case the peers
/// cannot connect to each other.
///
/// Some ISPs NAT implementation prevent direct incoming connections. If both peers are behind
/// such NATs, it means they literally will never be able to initiate a connection between the
/// two. So a fallback is necessary in such situations.
///
/// It is not computationally expensive, but all network traffic between the "peers" will go
/// through your own server, it is likely to massively increase your server's bandwidth usage,
/// upping your network bill.
///
/// It also defeats the main benefit of P2P for games: lower latency. Since this means all
/// the data will have to bounce through the TURN server.
///
/// TURN is a feature flag that can be removed through your Cargo.toml, if you do not wish
/// to provide a TURN server for your clients. (TODO: currently not)
///
/// See MDN https://developer.mozilla.org/en-US/docs/Web/API/WebRTC_API/Protocols
pub const TURN_PORT: u16 = 9004;

/// Unique identifier of signaling session that each user provides
/// when communicating with the signaling server.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct SessionId(u128);

impl SessionId {
    /// Wrap String into a SessionId struct
    pub fn new(inner: u128) -> Self {
        SessionId(inner)
    }

    /// Acquire the underlying type
    pub fn get(self) -> u128 {
        self.0
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Seid-{}", self.0)
    }
}
impl FromStr for SessionId {
    type Err = <u128 as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SessionId(s.parse()?))
    }
}

/// Unique identifier of each peer connected to signaling server
/// useful when communicating in one-to-many and many-to-many topologies.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct UserId(pub u64);

impl UserId {
    /// Wrap usize into a UserId struct
    pub fn new(inner: u64) -> Self {
        UserId(inner)
    }

    /// Acquire the underlying type
    pub fn into_inner(self) -> u64 {
        self.0
    }
}

impl From<u64> for UserId {
    fn from(val: u64) -> Self {
        UserId(val)
    }
}

impl Display for UserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
