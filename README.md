This crate is a fork of [wasm-peer] focusing on one-to-one communication with a
custom lobby management system. It also forcefully shuts websocket connections
when the peers are connected and attained the ideal connection configuration.
This is in contravention of the spec, as it requires a constant third party
"trusted" connection (aka: your centralized server). This implies that your
server must at all time have at least one open connection with every single
concurrent player. I don't have that kind of memory or budget, so I'll risk
angering the spec gods.

It also uses [msgpack] instead of json for message communication between peers,
since in the context of a game, we'd rather not have to constantly serialize and
deserialize floats or numbers.

The sessions server also integrate a STUN server, so that there is no external
dependencies.

In short:
- Integrated STUN server
- Removal of TURN handling
- only one-to-one sessions
- uses messagePack instead of json for communications
- various optimization (such as using u128 as session id rather than strings)

[wasm-peer]: https://github.com/wasm-peers/wasm-peers
[msgpack]: https://msgpack.org/

## Similar projects

* [matchbox](https://github.com/johanhelsing/matchbox#readme)

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as
defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Authors

Tomasz Karwowski  
[LinkedIn](https://www.linkedin.com/in/tomek-karwowski/)

## Acknowledgments

These projects helped me grasp WebRTC in Rust:

* [Yew WebRTC Chat](https://github.com/codec-abc/Yew-WebRTC-Chat)
* [WebRTC in Rust](https://github.com/Charles-Schleich/WebRTC-in-Rust)

Also, special thanks to the guys with whom I did my B.Eng. thesis.
