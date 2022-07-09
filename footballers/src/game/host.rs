use crate::game::constants::{
    BALL_GROUP, BALL_RADIUS, BALL_TOP_SPEED, GOAL_BREADTH, GOAL_DEPTH, GOAL_POSTS_GROUP, MAX_GOALS,
    PITCH_BOTTOM_LINE, PITCH_HEIGHT, PITCH_LEFT_LINE, PITCH_LINES_GROUP, PITCH_LINE_HEIGHT,
    PITCH_LINE_WIDTH, PITCH_RIGHT_LINE, PITCH_TOP_LINE, PITCH_VERTICAL_LINE_HEIGHT, PITCH_WIDTH,
    PLAYERS_GROUP, PLAYER_ACCELERATION, PLAYER_DIAMETER, PLAYER_RADIUS, PLAYER_TOP_SPEED,
    RESET_TIME, SHOOTING_DISTANCE, STADIUM_HEIGHT, STADIUM_WALLS_GROUP, STADIUM_WIDTH,
};
use crate::game::input::{local_player_input, PlayerInput};
use crate::game::utils::{Arbiter, Circle, Edge, Message, Player, Score};
use crate::game::{rendering, Game};
use crate::utils::global_window;
use rapier2d::dynamics::{
    CCDSolver, IntegrationParameters, IslandManager, JointSet, RigidBody, RigidBodyBuilder,
    RigidBodyHandle, RigidBodySet,
};
use rapier2d::geometry::{
    BroadPhase, ColliderBuilder, ColliderSet, InteractionGroups, NarrowPhase,
};
use rapier2d::pipeline::PhysicsPipeline;
use rapier2d::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_peers::one_to_one::NetworkManager;
use wasm_peers::{ConnectionType, SessionId};
use web_sys::CanvasRenderingContext2d;

pub struct HostGame {
    inner: Rc<RefCell<HostGameInner>>,
}

impl HostGame {
    pub fn new(
        session_id: SessionId,
        connection_type: ConnectionType,
        signaling_server_url: &str,
    ) -> HostGame {
        HostGame {
            inner: Rc::new(RefCell::new(HostGameInner::new(
                session_id,
                connection_type,
                signaling_server_url,
            ))),
        }
    }
}

impl Game for HostGame {
    fn init(&mut self) {
        let host_player = self.inner.borrow_mut().create_player(
            PITCH_LEFT_LINE + 2.0 * PLAYER_DIAMETER,
            STADIUM_HEIGHT / 2.0,
            true,
            1,
        );
        self.inner.borrow_mut().host_player = Some(host_player);

        let host_game = self.inner.clone();
        let on_open_callback = move || {
            let game_state = Message::GameInit {
                edges: host_game.borrow().get_edge_entities(),
                goal_posts: host_game.borrow().get_goal_posts_entities(),
                players: host_game.borrow().get_player_entities(),
                ball: host_game.borrow().get_ball_entity(),
            };
            let _ = host_game.borrow().mini_server.send_message(&game_state);
            host_game.borrow_mut().game_started = true;

            host_game.borrow_mut().oppo = Some(host_game.borrow_mut().create_player(
                PITCH_RIGHT_LINE - 2.0 * PLAYER_DIAMETER,
                STADIUM_HEIGHT / 2.0,
                false,
                1,
            ));
        };

        let host_game = self.inner.clone();
        let on_message_callback = move |input: PlayerInput| {
            if let Some(oppo) = &mut host_game.borrow_mut().oppo {
                oppo.set_input(input);
            }
        };

        self.inner.borrow().draw();

        self.inner
            .borrow_mut()
            .mini_server
            .start(10, on_open_callback, on_message_callback)
            .expect("network manager failed to start");
    }

    fn tick(&mut self) {
        self.inner.borrow_mut().tick();
    }

    fn ended(&self) -> bool {
        self.inner.borrow().get_game_ended()
    }
}

pub struct HostGameInner {
    host_player: Option<Player>,
    oppo: Option<Player>,
    edges: Vec<Edge>,
    goal_posts: Vec<Circle>,
    ball_body_handle: RigidBodyHandle,
    arbiter: Arbiter,

    // required by networking crate
    mini_server: NetworkManager,
    game_started: bool,

    // stuff required by physics engine
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    joint_set: JointSet,
    ccd_solver: CCDSolver,
    physics_hooks: (),
    event_handler: (),
    // drawing stuff
    context: CanvasRenderingContext2d,
    player_input: Rc<RefCell<PlayerInput>>,
}

impl HostGameInner {
    pub(self) fn new(
        session_id: SessionId,
        connection_type: ConnectionType,
        signaling_server_url: &str,
    ) -> HostGameInner {
        let mini_server = NetworkManager::new(signaling_server_url, session_id, connection_type)
            .expect("failed to create network manager");

        let mut rigid_body_set = RigidBodySet::new();
        let mut collider_set = ColliderSet::new();

        let edges = HostGameInner::create_pitch_lines(&mut collider_set);
        let goal_posts = HostGameInner::create_goals_posts(&mut collider_set);
        HostGameInner::create_stadium_walls(&mut collider_set);

        let ball_body_handle = HostGameInner::create_ball(&mut rigid_body_set, &mut collider_set);

        let document = global_window().document().unwrap();
        let context = {
            let canvas = document.get_element_by_id("canvas").unwrap();
            let canvas: web_sys::HtmlCanvasElement = canvas
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .map_err(|_| ())
                .unwrap();

            canvas
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<web_sys::CanvasRenderingContext2d>()
                .unwrap()
        };
        context.set_text_align("center");
        context.set_text_baseline("middle");

        HostGameInner {
            mini_server,
            game_started: false,
            host_player: None,
            oppo: None,
            edges,
            goal_posts,
            ball_body_handle,
            arbiter: Arbiter::new(),
            rigid_body_set,
            collider_set,
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            joint_set: JointSet::new(),
            ccd_solver: CCDSolver::new(),
            physics_hooks: (),
            event_handler: (),
            context,
            player_input: local_player_input(),
        }
    }

    pub(self) fn tick(&mut self) {
        self.check_timer();
        self.host_player
            .as_mut()
            .unwrap()
            .set_input(*self.player_input.borrow());
        self.advance_physic_tick();

        HostGameInner::limit_speed(
            &mut self.rigid_body_set[self.ball_body_handle],
            BALL_TOP_SPEED,
        );

        self.physics_pipeline.step(
            &vector![0.0, 0.0],
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.joint_set,
            &mut self.ccd_solver,
            &self.physics_hooks,
            &self.event_handler,
        );

        self.host_send_state();
        self.draw();
    }

    fn create_pitch_lines(collider_set: &mut ColliderSet) -> Vec<Edge> {
        let mut edges = Vec::new();
        let mut create_line_closure = |width, height, x, y, white, membership, filter| {
            let cuboid_collider = ColliderBuilder::cuboid(width / 2.0, height / 2.0)
                .collision_groups(InteractionGroups::new(membership, filter))
                .translation(vector![x, y])
                .build();
            edges.push(Edge::new(
                cuboid_collider.translation().x,
                cuboid_collider.translation().y,
                width,
                height,
                white,
            ));
            collider_set.insert(cuboid_collider);
        };

        // left higher pitch line
        create_line_closure(
            PITCH_LINE_WIDTH,
            PITCH_VERTICAL_LINE_HEIGHT,
            PITCH_LEFT_LINE,
            (STADIUM_HEIGHT - GOAL_BREADTH - PITCH_VERTICAL_LINE_HEIGHT) / 2.0,
            true,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );
        // left lower pitch line
        create_line_closure(
            PITCH_LINE_WIDTH,
            PITCH_VERTICAL_LINE_HEIGHT,
            PITCH_LEFT_LINE,
            (STADIUM_HEIGHT + GOAL_BREADTH + PITCH_VERTICAL_LINE_HEIGHT) / 2.0,
            true,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );
        // left goal
        create_line_closure(
            PITCH_LINE_WIDTH,
            GOAL_BREADTH,
            PITCH_LEFT_LINE - GOAL_DEPTH,
            STADIUM_HEIGHT / 2.0,
            false,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );
        create_line_closure(
            GOAL_DEPTH,
            PITCH_LINE_HEIGHT,
            PITCH_LEFT_LINE - GOAL_DEPTH / 2.0,
            (STADIUM_HEIGHT - GOAL_BREADTH) / 2.0,
            false,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );
        create_line_closure(
            GOAL_DEPTH,
            PITCH_LINE_HEIGHT,
            PITCH_LEFT_LINE - GOAL_DEPTH / 2.0,
            (STADIUM_HEIGHT + GOAL_BREADTH) / 2.0,
            false,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );

        // right higher pitch line
        create_line_closure(
            PITCH_LINE_WIDTH,
            PITCH_VERTICAL_LINE_HEIGHT,
            PITCH_RIGHT_LINE,
            (STADIUM_HEIGHT - GOAL_BREADTH - PITCH_VERTICAL_LINE_HEIGHT) / 2.0,
            true,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );
        // right lower pitch line
        create_line_closure(
            PITCH_LINE_WIDTH,
            PITCH_VERTICAL_LINE_HEIGHT,
            PITCH_RIGHT_LINE,
            (STADIUM_HEIGHT + GOAL_BREADTH + PITCH_VERTICAL_LINE_HEIGHT) / 2.0,
            true,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );
        // right goal
        create_line_closure(
            PITCH_LINE_WIDTH,
            GOAL_BREADTH,
            PITCH_RIGHT_LINE + GOAL_DEPTH,
            STADIUM_HEIGHT / 2.0,
            false,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );
        create_line_closure(
            GOAL_DEPTH,
            PITCH_LINE_HEIGHT,
            PITCH_RIGHT_LINE + GOAL_DEPTH / 2.0,
            (STADIUM_HEIGHT - GOAL_BREADTH) / 2.0,
            false,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );
        create_line_closure(
            GOAL_DEPTH,
            PITCH_LINE_HEIGHT,
            PITCH_RIGHT_LINE + GOAL_DEPTH / 2.0,
            (STADIUM_HEIGHT + GOAL_BREADTH) / 2.0,
            false,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );

        // top pitch line`
        create_line_closure(
            PITCH_WIDTH,
            PITCH_LINE_HEIGHT,
            STADIUM_WIDTH / 2.0,
            PITCH_TOP_LINE,
            true,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );

        // bottom pitch line
        create_line_closure(
            PITCH_WIDTH,
            PITCH_LINE_HEIGHT,
            STADIUM_WIDTH / 2.0,
            PITCH_BOTTOM_LINE,
            true,
            PITCH_LINES_GROUP,
            PITCH_LINES_GROUP,
        );

        edges
    }

    fn create_goals_posts(collider_set: &mut ColliderSet) -> Vec<Circle> {
        let mut goal_posts = Vec::new();

        let mut create_post_closure = |x, y, red| {
            let ball_collider = ColliderBuilder::ball(BALL_RADIUS)
                .collision_groups(InteractionGroups::new(GOAL_POSTS_GROUP, GOAL_POSTS_GROUP))
                .translation(vector![x, y])
                .build();
            goal_posts.push(Circle::new(
                ball_collider.translation().x,
                ball_collider.translation().y,
                BALL_RADIUS,
                red,
                -1,
            ));
            collider_set.insert(ball_collider);
        };
        // left red goal
        create_post_closure(
            PITCH_LEFT_LINE,
            PITCH_TOP_LINE + PITCH_HEIGHT / 2.0 - GOAL_BREADTH / 2.0,
            true,
        );
        create_post_closure(
            PITCH_LEFT_LINE,
            PITCH_TOP_LINE + PITCH_HEIGHT / 2.0 + GOAL_BREADTH / 2.0,
            true,
        );

        // right blue goal
        create_post_closure(
            PITCH_RIGHT_LINE,
            PITCH_TOP_LINE + PITCH_HEIGHT / 2.0 - GOAL_BREADTH / 2.0,
            false,
        );
        create_post_closure(
            PITCH_RIGHT_LINE,
            PITCH_TOP_LINE + PITCH_HEIGHT / 2.0 + GOAL_BREADTH / 2.0,
            false,
        );

        goal_posts
    }

    fn create_stadium_walls(collider_set: &mut ColliderSet) {
        let mut create_wall_closure = |width, height, x, y| {
            let cuboid_collider = ColliderBuilder::cuboid(width / 2.0, height / 2.0)
                .collision_groups(InteractionGroups::new(
                    STADIUM_WALLS_GROUP,
                    STADIUM_WALLS_GROUP,
                ))
                .translation(vector![x, y])
                .build();
            collider_set.insert(cuboid_collider);
        };
        // left stadium wall
        create_wall_closure(0.0, STADIUM_HEIGHT, 0.0, STADIUM_HEIGHT / 2.0);

        // right stadium wall
        create_wall_closure(0.0, STADIUM_HEIGHT, STADIUM_WIDTH, STADIUM_HEIGHT / 2.0);

        // top stadium wall
        create_wall_closure(STADIUM_WIDTH, 0.0, STADIUM_WIDTH / 2.0, 0.0);

        // bottom stadium wall
        create_wall_closure(STADIUM_WIDTH, 0.0, STADIUM_WIDTH / 2.0, STADIUM_HEIGHT);
    }

    fn create_player(&mut self, x: f32, y: f32, is_red: bool, number: usize) -> Player {
        const COLLISION_GROUP: u32 =
            PLAYERS_GROUP | STADIUM_WALLS_GROUP | BALL_GROUP | GOAL_POSTS_GROUP;
        let player_rigid_body = RigidBodyBuilder::new_dynamic()
            .linear_damping(1.0)
            .translation(vector![x, y])
            .build();
        let player_rigid_body = Rc::new(RefCell::new(player_rigid_body));
        let player_collider = ColliderBuilder::ball(PLAYER_RADIUS)
            .collision_groups(InteractionGroups::new(COLLISION_GROUP, COLLISION_GROUP))
            .restitution(0.7)
            .build();
        let player_body_handle: RigidBodyHandle = self
            .rigid_body_set
            .insert(player_rigid_body.borrow().to_owned());
        self.collider_set.insert_with_parent(
            player_collider,
            player_body_handle,
            &mut self.rigid_body_set,
        );
        Player::new(player_body_handle, PLAYER_RADIUS, is_red, number)
    }

    fn create_ball(
        rigid_body_set: &mut RigidBodySet,
        collider_set: &mut ColliderSet,
    ) -> RigidBodyHandle {
        const COLLISION_GROUP: u32 =
            BALL_GROUP | PLAYERS_GROUP | PITCH_LINES_GROUP | GOAL_POSTS_GROUP;

        let ball_rigid_body = RigidBodyBuilder::new_dynamic()
            .linear_damping(0.3)
            .translation(vector![STADIUM_WIDTH / 2.0, STADIUM_HEIGHT / 2.0])
            .build();
        let ball_rigid_body = Rc::new(RefCell::new(ball_rigid_body));
        let ball_collider = ColliderBuilder::ball(BALL_RADIUS)
            .density(0.5)
            .collision_groups(InteractionGroups::new(COLLISION_GROUP, COLLISION_GROUP))
            .restitution(0.7)
            .build();
        let ball_body_handle: RigidBodyHandle =
            rigid_body_set.insert(ball_rigid_body.borrow().to_owned());
        collider_set.insert_with_parent(ball_collider, ball_body_handle, rigid_body_set);

        ball_body_handle
    }

    fn host_send_state(&mut self) {
        let game_state = if self.arbiter.send_score_message {
            self.arbiter.send_score_message = false;
            Message::GoalScored {
                red_scored: self.get_red_scored(),
                score: self.get_score(),
            }
        } else {
            Message::GameState {
                players: self.get_player_entities(),
                ball: self.get_ball_entity(),
            }
        };
        self.mini_server.send_message(&game_state);
    }

    fn advance_physic_tick(&mut self) {
        let players = self.host_player.iter_mut().chain(self.oppo.iter_mut());
        for player in players {
            let player_last_tick_shot = player.last_tick_shot;
            let input = player.get_input();
            let body_handle = player.rigid_body_handle;

            if input.shoot {
                if !player_last_tick_shot {
                    let px;
                    let py;
                    {
                        let player_body = &self.rigid_body_set[body_handle];
                        px = player_body.translation().x;
                        py = player_body.translation().y;
                    }

                    let ball_body = &mut self.rigid_body_set[self.ball_body_handle];
                    let bx = ball_body.translation().x;
                    let by = ball_body.translation().y;

                    let dx = bx - px;
                    let dy = by - py;
                    let dist_sqr = dx * dx + dy * dy;
                    if dist_sqr <= SHOOTING_DISTANCE * SHOOTING_DISTANCE {
                        let angle = crate::game::utils::angle(px, py, bx, by);
                        let x_speed =
                            BALL_TOP_SPEED * (std::f32::consts::PI * (angle / 180.0)).cos();
                        let y_speed =
                            BALL_TOP_SPEED * (std::f32::consts::PI * (angle / 180.0)).sin();
                        ball_body.set_linvel(vector![x_speed, y_speed], true);
                    }
                    player.set_last_tick_shot(true);
                }
            } else {
                player.set_last_tick_shot(false);
            }

            let player_body = &mut self.rigid_body_set[body_handle];

            if input.up {
                player_body.apply_impulse(vector![0.0, -PLAYER_ACCELERATION], true);
            } else if input.down {
                player_body.apply_impulse(vector![0.0, PLAYER_ACCELERATION], true);
            }

            if input.left {
                player_body.apply_impulse(vector![-PLAYER_ACCELERATION, 0.0], true);
            } else if input.right {
                player_body.apply_impulse(vector![PLAYER_ACCELERATION, 0.0], true);
            }

            HostGameInner::limit_speed(player_body, PLAYER_TOP_SPEED);
        }
    }

    fn limit_speed(rigid_body: &mut RigidBody, top_speed: f32) {
        let x_speed = rigid_body.linvel().x;
        let y_speed = rigid_body.linvel().y;
        let speed = f32::sqrt(x_speed * x_speed + y_speed * y_speed);
        if speed > top_speed {
            let speed_normalized = rigid_body.linvel().normalize();
            rigid_body.set_linvel(
                vector![
                    speed_normalized.x * top_speed,
                    speed_normalized.y * top_speed
                ],
                true,
            );
        }
    }

    fn check_timer(&mut self) {
        if self.arbiter.game_ended {
            return;
        }
        if self.arbiter.reset_timer > 0 {
            self.timer_tick();
        } else if self.goal_scored() {
            self.arbiter.reset_timer = RESET_TIME;
        }
    }

    fn goal_scored(&mut self) -> bool {
        let ball_body = &mut self.rigid_body_set[self.ball_body_handle];
        let x = ball_body.translation().x;
        if x < PITCH_LEFT_LINE {
            self.arbiter.set_blue_scored();
            true
        } else if x > PITCH_RIGHT_LINE {
            self.arbiter.set_red_scored();
            true
        } else {
            false
        }
    }

    fn timer_tick(&mut self) {
        self.arbiter.reset_timer -= 1;
        if self.arbiter.reset_timer == 0 {
            self.arbiter.reset_who_scored();
            self.check_ending();
            self.reset_game();
        }
    }

    fn check_ending(&mut self) {
        if self.arbiter.red_score == MAX_GOALS || self.arbiter.blue_score == MAX_GOALS {
            self.arbiter.game_ended = true;
            self.mini_server.send_message(&Message::GameEnded);
        }
    }

    fn reset_game(&mut self) {
        {
            let ball_body = &mut self.rigid_body_set[self.ball_body_handle];
            ball_body.set_position(
                Isometry::new(vector![STADIUM_WIDTH / 2.0, STADIUM_HEIGHT / 2.0], 0.0),
                false,
            );
            ball_body.set_linvel(vector![0.0, 0.0], false);
        }

        if let Some(oppo) = &mut self.oppo {
            oppo.reset_position(&mut self.rigid_body_set, 0.0, 0.0);
        }
        if let Some(player) = &mut self.host_player {
            player.reset_position(&mut self.rigid_body_set, 0.0, 0.0);
        }
    }

    fn players(&self) -> impl Iterator<Item = &Player> {
        self.host_player.iter().chain(self.oppo.iter())
    }
    fn get_player_entities(&self) -> Vec<Circle> {
        self.players()
            .map(|p| p.to_circle(&self.rigid_body_set))
            .collect()
    }

    fn get_ball_entity(&self) -> Circle {
        let brb = &self.rigid_body_set[self.ball_body_handle];
        Circle::new(
            brb.translation().x,
            brb.translation().y,
            BALL_RADIUS,
            false,
            -1,
        )
    }

    fn get_edge_entities(&self) -> Vec<Edge> {
        self.edges.clone()
    }

    fn get_goal_posts_entities(&self) -> Vec<Circle> {
        self.goal_posts.clone()
    }

    fn get_red_scored(&self) -> bool {
        self.arbiter.red_scored
    }

    fn get_blue_scored(&self) -> bool {
        self.arbiter.blue_scored
    }

    fn get_score(&self) -> Score {
        Score::new(self.arbiter.red_score, self.arbiter.blue_score)
    }

    fn get_game_ended(&self) -> bool {
        self.arbiter.game_ended
    }

    fn draw(&self) {
        rendering::draw_stadium(&self.context, STADIUM_WIDTH as f64, STADIUM_HEIGHT as f64);
        rendering::draw_pitch(
            &self.context,
            &self.edges,
            PITCH_LEFT_LINE as f64,
            PITCH_RIGHT_LINE as f64,
            PITCH_TOP_LINE as f64,
            PITCH_BOTTOM_LINE as f64,
            PITCH_LINE_WIDTH as f64,
            STADIUM_WIDTH as f64,
            STADIUM_HEIGHT as f64,
            GOAL_BREADTH as f64,
        );
        rendering::draw_goals(&self.context, &self.goal_posts);
        rendering::draw_score(
            &self.context,
            &self.get_score(),
            STADIUM_WIDTH as f64,
            PITCH_TOP_LINE as f64,
        );
        rendering::draw_players(&self.context, &self.get_player_entities());
        rendering::draw_ball(&self.context, &self.get_ball_entity());
        if self.get_red_scored() {
            rendering::draw_red_scored(&self.context, STADIUM_WIDTH as f64, STADIUM_HEIGHT as f64);
        }
        if self.get_blue_scored() {
            rendering::draw_blue_scored(&self.context, STADIUM_WIDTH as f64, STADIUM_HEIGHT as f64);
        }
        if self.get_game_ended() {
            rendering::draw_game_ended(
                &self.context,
                &self.get_score(),
                STADIUM_WIDTH as f64,
                STADIUM_HEIGHT as f64,
            );
        }
    }
}
