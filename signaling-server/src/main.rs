use log::LevelFilter;
use simplelog::{Config, TermLogger, TerminalMode};
use std::{env, std::net::SocketAddr, std::str::FromStr, sync::Arc, time::Duration};
use tokio::net::UdpSocket;
use warp::Filter;

use wasm_peers_protocol::{STUN_PORT, TURN_PORT, WS_PORT};
use wasm_peers_signaling_server::one_to_one;

use turn::{
    auth::{self, AuthHandler},
    relay::relay_static::{self, RelayAddressGeneratorStatic},
    server::{self, config},
};

struct MyAuthHandler {
    cred_map: HashMap<String, Vec<u8>>,
}

impl MyAuthHandler {
    fn new(cred_map: HashMap<String, Vec<u8>>) -> Self {
        MyAuthHandler { cred_map }
    }
}

impl AuthHandler for MyAuthHandler {
    fn auth_handle(
        &self,
        username: &str,
        _realm: &str,
        _src_addr: SocketAddr,
    ) -> Result<Vec<u8>, Error> {
        if let Some(pw) = self.cred_map.get(username) {
            //log::debug!("username={}, password={:?}", username, pw);
            Ok(pw.to_vec())
        } else {
            Err(Error::ErrFakeErr)
        }
    }
}
fn port_overlap(addr: &SocketAddr) -> bool {
    [STUN_PORT, TURN_PORT, WS_PORT].contains(&addr.port())
}
#[tokio::main]
async fn main() {
    TermLogger::init(LevelFilter::Debug, Config::default(), TerminalMode::Mixed).unwrap();

    let connections = one_to_one::Connections::default();
    let connections = warp::any().map(move || connections.clone());

    let sessions = one_to_one::Sessions::default();
    let sessions = warp::any().map(move || sessions.clone());

    let signaling_channel = warp::path("one-to-one")
        .and(warp::ws())
        .and(connections)
        .and(sessions)
        .map(|ws: warp::ws::Ws, connections, sessions| {
            ws.on_upgrade(move |socket| one_to_one::user_connected(socket, connections, sessions))
        });

    let fallback = || "127.0.0.1:9000".to_string();
    let address = env::args().nth(1).unwrap_or_else(fallback);
    let address = SocketAddr::from_str(&address).expect("invalid IP address provided");
    if port_overlap(&address) {
        panic!("{address}'s port overlap with one of the protocol's predefined port, the port for the default server that serves the static wasm and html files should be distinct from the rest.");
    }

    let conn = Arc::new(UdpSocket::bind(format!(":{TURN_PORT}")).await?);
    println!("listening {}...", conn.local_addr()?);
    tokio::spawn(Server::new(ServerConfig {
        conn_configs: vec![ConnConfig {
            conn,
            relay_addr_generator: Box::new(RelayAddressGeneratorStatic {
                relay_address: IpAddr::from_str(public_ip)?,
                address: "0.0.0.0".to_owned(),
                net: Arc::new(Net::new(None)),
            }),
        }],
        realm: "".to_owned(),
        auth_handler: Arc::new(MyAuthHandler::new(cred_map)),
        channel_bind_timeout: Duration::from_secs(0),
    }));
    warp::serve(signaling_channel).run(address).await;
}
