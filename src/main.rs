mod gameplay;
mod generation;
mod graphics;
mod spritesheet;

use gameplay::{LogicalTransition, LogicalWorld};
use generation::generate_level;
use ggez::{
	conf::{WindowMode, WindowSetup},
	event::{run, EventHandler},
	glam::IVec2,
	graphics::{Canvas, Color, Sampler},
	input::keyboard::KeyInput,
	winit::event::VirtualKeyCode,
	Context, ContextBuilder, GameResult,
};
use graphics::{Camera, GraphicalWorld};
use spritesheet::SpritesheetStuff;

enum Phase {
	/// The player may take their time then make a move.
	WaitingForPlayerToMakeAMove,
	/// Some animations are still playing in `Game::graphical_world`.
	/// If all animations are finished, then the next transition in the vec here is to
	/// be applied.
	WaitingForAnimationsToFinish(Vec<LogicalTransition>),
}

/// The whole game state.
struct Game {
	/// The current logical state of the world.
	logical_world: LogicalWorld,
	/// All previous states of the world, from oldest to most recent.
	previous_logical_worlds: Vec<LogicalWorld>,
	phase: Phase,
	graphical_world: GraphicalWorld,
	camera: Camera,
	spritesheet_stuff: SpritesheetStuff,
}

impl Game {
	fn new(ctx: &mut Context) -> GameResult<Game> {
		let lw = generate_level();
		let gw = GraphicalWorld::from_logical_world(&lw);
		let spritesheet_stuff = SpritesheetStuff::new(ctx)?;
		let phase = Phase::WaitingForPlayerToMakeAMove;
		let mut camera = Camera::new();
		camera.set_initial_target(&gw.info_for_camera);
		Ok(Game {
			logical_world: lw,
			previous_logical_worlds: vec![],
			phase,
			graphical_world: gw,
			camera,
			spritesheet_stuff,
		})
	}

	fn player_move(&mut self, direction: IVec2) {
		if matches!(self.phase, Phase::WaitingForPlayerToMakeAMove) && self.logical_world.has_player()
		{
			let mut transition = self.logical_world.player_move(direction);
			self.previous_logical_worlds.push(self.logical_world.clone());
			self.logical_world = transition.resulting_lw.clone();
			self.graphical_world = GraphicalWorld::from_logical_world_transition(&transition);
			self.camera.set_target(&self.graphical_world.info_for_camera);

			// Play all the moves of everything that is not a player up until the player's next turn.
			transition.resulting_lw.give_move_token_to_agents();
			let mut transitions = vec![];
			while let Some(next_transition) = transition.resulting_lw.handle_move_for_one_agent() {
				transitions.push(next_transition.clone());
				transition = next_transition;
			}
			self.phase = Phase::WaitingForAnimationsToFinish(transitions);
		}
	}

	fn redo(&mut self) {
		if matches!(self.phase, Phase::WaitingForPlayerToMakeAMove) {
			if let Some(previous_lw) = self.previous_logical_worlds.pop() {
				let redo_count = self.logical_world.redo_count;
				if redo_count >= 1 {
					self.logical_world = previous_lw;
					self.logical_world.redo_count = redo_count - 1;
					self.graphical_world = GraphicalWorld::from_logical_world(&self.logical_world);
					self.camera.set_target(&self.graphical_world.info_for_camera);
				}
			}
		}
	}
}

impl EventHandler for Game {
	fn update(&mut self, ctx: &mut Context) -> GameResult {
		loop {
			let no_more_animations = !self.graphical_world.has_animation();
			if no_more_animations {
				if let Phase::WaitingForAnimationsToFinish(next_tranitions) = &mut self.phase {
					if !next_tranitions.is_empty() {
						let transition = next_tranitions.remove(0);
						self.logical_world = transition.resulting_lw.clone();
						self.graphical_world = GraphicalWorld::from_logical_world_transition(&transition);
						self.camera.set_target(&self.graphical_world.info_for_camera);
					} else {
						self.phase = Phase::WaitingForPlayerToMakeAMove;
					}
				} else {
					break;
				}
			} else {
				break;
			}
		}

		self.camera.animate(ctx.time.delta());

		Ok(())
	}

	fn key_down_event(&mut self, ctx: &mut Context, input: KeyInput, _repeated: bool) -> GameResult {
		use VirtualKeyCode as K;
		if let Some(keycode) = input.keycode {
			match keycode {
				K::Escape => ctx.request_quit(),
				K::Z | K::W | K::Up => self.player_move(IVec2::new(0, -1)),
				K::Q | K::A | K::Left => self.player_move(IVec2::new(-1, 0)),
				K::S | K::Down => self.player_move(IVec2::new(0, 1)),
				K::D | K::Right => self.player_move(IVec2::new(1, 0)),
				K::R | K::Back => self.redo(),
				_ => {},
			}
		}
		Ok(())
	}

	fn draw(&mut self, ctx: &mut Context) -> GameResult {
		let mut canvas = Canvas::from_frame(ctx, Color::BLACK);
		canvas.set_sampler(Sampler::nearest_clamp());
		self.graphical_world.draw(ctx, &mut canvas, &self.spritesheet_stuff, &self.camera)?;
		canvas.finish(ctx)?;
		Ok(())
	}
}

fn main() -> GameResult {
	let (mut ctx, event_loop) = ContextBuilder::new("PushDg", "Anima :3")
		.window_setup(WindowSetup::default().title("PushDg").vsync(true).srgb(false))
		.window_mode(WindowMode::default().dimensions(800.0, 800.0))
		.build()
		.unwrap();
	let game = Game::new(&mut ctx)?;
	run(ctx, event_loop, game);
}
