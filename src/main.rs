mod gameplay;
mod graphics;
mod spritesheet;

use gameplay::{LogicalTransition, LogicalWorld};
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
	phase: Phase,
	graphical_world: GraphicalWorld,
	camera: Camera,
	spritesheet_stuff: SpritesheetStuff,
}

impl Game {
	fn new(ctx: &mut Context) -> GameResult<Game> {
		let lw = LogicalWorld::new_test();
		let gw = GraphicalWorld::from_logical_world(&lw);
		let spritesheet_stuff = SpritesheetStuff::new(ctx)?;
		let phase = Phase::WaitingForPlayerToMakeAMove;
		let camera = Camera::new();
		Ok(Game {
			logical_world: lw,
			phase,
			graphical_world: gw,
			camera,
			spritesheet_stuff,
		})
	}

	fn player_move(&mut self, direction: IVec2) {
		if matches!(self.phase, Phase::WaitingForPlayerToMakeAMove) {
			let mut transition = self.logical_world.player_move(direction);
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
}

impl EventHandler for Game {
	fn update(&mut self, ctx: &mut Context) -> GameResult {
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
