use crate::components::utils;
use crate::game::{FootballersGame, Game, HostGame, GAME_CANVAS_HEIGHT, GAME_CANVAS_WIDTH};
use crate::js_interface;
use crate::utils::global_window;
use log::{error, info};
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_peers::{ConnectionType, SessionId};
use web_sys::HtmlCanvasElement;
use yew::{html, Component, Context, Html, NodeRef};

#[derive(Serialize, Deserialize)]
pub struct GameQuery {
    pub session_id: SessionId,
    pub is_host: bool,
}

#[derive(Debug)]
pub struct GameInit {
    pub session_id: SessionId,
    pub signaling_server: String,
    pub username: String,
    pub credential: String,
}
impl GameQuery {
    pub(crate) fn new(session_id: SessionId, is_host: bool) -> Self {
        GameQuery {
            session_id,
            is_host,
        }
    }
}

pub enum GameMsg {
    CopyLink,
    Init,
    Tick,
}

pub(crate) struct GameComponent {
    session_id: SessionId,
    canvas: NodeRef,
    game: Option<FootballersGame>,
    tick_callback: Closure<dyn FnMut()>,
}

impl Component for GameComponent {
    type Message = GameMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let query_params = utils::get_query_params();
        let session_id = if let Some(session_string) = query_params.get("session_id") {
            SessionId::new(session_string.parse().unwrap())
        } else {
            todo!("Handle no session strings")
        };
        let canvas = NodeRef::default();
        let tick_callback = {
            let link = ctx.link().clone();
            Closure::wrap(Box::new(move || link.send_message(GameMsg::Tick)) as Box<dyn FnMut()>)
        };
        ctx.link().send_message(GameMsg::Init);
        Self {
            session_id,
            canvas,
            game: None,
            tick_callback,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            GameMsg::CopyLink => {
                if let Err(e) = copy_link(&self.session_id) {
                    error!("{e:?}");
                }
                false
            }
            GameMsg::Init => {
                let init = GameInit {
                    session_id: self.session_id,
                    signaling_server: js_interface::server(),
                    username: js_interface::turn_username(),
                    credential: js_interface::turn_credential(),
                };
                info!("{init:?}");
                self.game = Some(init_game(self.canvas.clone(), init));
                ctx.link().send_message(GameMsg::Tick);
                false
            }
            GameMsg::Tick => {
                match self.game.as_mut() {
                    Some(game) => {
                        game.tick();
                        if !game.ended() {
                            if let Err(error) = global_window().request_animation_frame(
                                self.tick_callback.as_ref().unchecked_ref(),
                            ) {
                                error!("Failed requesting next animation frame: {error:?}");
                            }
                        }
                    }
                    None => {
                        error!("No initialized game object yet.");
                    }
                }
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let width = GAME_CANVAS_WIDTH.to_string();
        let height = GAME_CANVAS_HEIGHT.to_string();
        let onclick = ctx.link().callback(|_| GameMsg::CopyLink);
        html! {
            <div class="px-3">
                <canvas id="canvas" { width } { height } ref={ self.canvas.clone() }></canvas>
                <p class="lead">{ "Use WASD to move, SPACE to shoot the ball." }</p>
                <p class="lead">{ "Session id:" } { &self.session_id }</p>
                <button id="game_link_button" { onclick }>{ "Copy shareable link" }</button>
            </div>
        }
    }
}

fn init_game(canvas_node: NodeRef, settings: GameInit) -> FootballersGame {
    let context = {
        let canvas = canvas_node
            .cast::<HtmlCanvasElement>()
            .expect("no canvas element on page yet");
        canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap()
    };
    context.set_text_align("center");
    context.set_text_baseline("middle");

    let connection_type = ConnectionType::StunAndTurn {
        host: settings.signaling_server.clone(),
        username: settings.username.clone(),
        credential: settings.credential.clone(),
    };
    let mut game = HostGame::new(
        settings.session_id,
        connection_type,
        &settings.signaling_server,
    );
    game.init();
    game
}

fn copy_link(session_id: &SessionId) -> Result<(), JsValue> {
    let window = global_window();
    let clipboard = window
        .navigator()
        .clipboard()
        .ok_or_else(|| JsValue::from("acquiring clipboard failed"))?;
    let location = window.location();
    let origin = location.origin()?;
    let pathname = location.pathname()?;
    let _promise = clipboard.write_text(&format!("{origin}{pathname}?session_id={session_id}"));
    Ok(())
}
