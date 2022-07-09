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
