use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt, TryFutureExt};
use log::{error, info, warn};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

use wasm_peers_protocol::one_to_one::SignalMessage;
use wasm_peers_protocol::{SessionId, UserId};

pub struct Session {
    pub first: Option<UserId>,
    pub second: Option<UserId>,
    pub offer_received: bool,
}

pub type Connections = Arc<RwLock<HashMap<UserId, mpsc::UnboundedSender<Message>>>>;
pub type Sessions = Arc<RwLock<HashMap<SessionId, Session>>>;

static NEXT_USER_ID: AtomicU64 = AtomicU64::new(1);

pub async fn user_connected(ws: WebSocket, connections: Connections, sessions: Sessions) {
    let user_id = UserId::new(NEXT_USER_ID.fetch_add(1, Ordering::Relaxed));
    info!("new user connected: {:?}", user_id);

    let (mut user_ws_tx, mut user_ws_rx) = ws.split();

    let (tx, rx) = mpsc::unbounded_channel();
    let mut rx = UnboundedReceiverStream::new(rx);

    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            user_ws_tx
                .send(message)
                .unwrap_or_else(|e| eprintln!("websocket send error: {}", e))
                .await;
        }
    });
    connections.write().await.insert(user_id, tx);

    while let Some(result) = user_ws_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error (id={:?}): {}", user_id, e);
                break;
            }
        };
        user_message(user_id, msg, &connections, &sessions).await;
    }
    eprintln!("user disconnected: {:?}", user_id);
    user_disconnected(user_id, &connections, &sessions).await;
}

async fn user_message(
    user_id: UserId,
    msg: Message,
    connections: &Connections,
    sessions: &Sessions,
) {
    use SignalMessage::{IceCandidate, SdpAnswer, SdpOffer};
    let request = match rmp_serde::from_slice::<SignalMessage>(msg.as_bytes()) {
        Ok(request) => {
            info!("message received from user {:?}: {:?}", user_id, request);
            request
        }
        Err(error) => {
            error!("An error occurred: {:?}", error);
            return;
        }
    };
    match &request {
        SignalMessage::SessionJoin(session_id) => {
            match sessions.write().await.entry(*session_id) {
                // on first user in session - create session object and store connecting user id
                Entry::Vacant(entry) => {
                    entry.insert(Session {
                        first: Some(user_id),
                        second: None,
                        offer_received: false,
                    });
                }
                // on second user - add him to existing session and notify users that session is ready
                Entry::Occupied(mut entry) => {
                    entry.get_mut().second = Some(user_id);
                    let first_response = SignalMessage::SessionReady(*session_id);
                    let second_response = SignalMessage::SessionReady(*session_id);
                    let first_response = rmp_serde::to_vec(&first_response).unwrap();
                    let second_response = rmp_serde::to_vec(&second_response).unwrap();

                    let connections_reader = connections.read().await;
                    if let Some(first_id) = &entry.get().first {
                        let first_tx = connections_reader.get(first_id).unwrap();
                        first_tx.send(Message::binary(first_response)).unwrap();
                        let second_tx = connections_reader.get(&user_id).unwrap();
                        second_tx.send(Message::binary(second_response)).unwrap();
                    }
                }
            }
        }
        // pass offer and answer to the other user in session without changing anything
        message @ (SdpOffer(id, _) | SdpAnswer(id, _) | IceCandidate(id, _)) => {
            let mut lock = sessions.write().await;
            let session = match lock.get_mut(id) {
                Some(session) => session,
                None => {
                    error!("No such session: {id:?}");
                    return;
                }
            };
            if session.offer_received {
                warn!("offer already sent by the the peer, ignoring the 2nd offer: {id:?}");
            } else {
                session.offer_received = true;
            }

            let recipient = if session.first.is_some() {
                session.second
            } else {
                session.first
            };
            match recipient {
                Some(recipient_id) => {
                    let response = message;
                    let response = rmp_serde::to_vec(&response).unwrap();
                    let connections_reader = connections.read().await;
                    let recipient_tx = connections_reader.get(&recipient_id).unwrap();

                    recipient_tx.send(Message::binary(response)).unwrap();
                }
                None => {
                    error!("Missing second user in session: {:?}", &id);
                }
            }
        }
        SignalMessage::SessionReady(_) | SignalMessage::Error(..) => {}
    }
}

async fn user_disconnected(user_id: UserId, connections: &Connections, sessions: &Sessions) {
    let mut session_to_delete = None;
    for (session_id, session) in sessions.write().await.iter_mut() {
        if session.first == Some(user_id) {
            session.first = None;
        } else if session.second == Some(user_id) {
            session.second = None;
        }
        if session.first == None && session.second == None {
            session_to_delete = Some(*session_id);
        }
    }
    // remove session if it's empty
    if let Some(session_id) = session_to_delete {
        sessions.write().await.remove(&session_id);
    }
    connections.write().await.remove(&user_id);
}
