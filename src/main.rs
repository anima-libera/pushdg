use ggez::{
	conf::{WindowMode, WindowSetup},
	event::{run, EventHandler},
	graphics::{Canvas, Color, Sampler},
	ContextBuilder, GameResult,
};

struct Game {}

impl Game {
	fn new(_ctx: &mut ggez::Context) -> GameResult<Game> {
		Ok(Game {})
	}
}

impl EventHandler for Game {
	fn update(&mut self, _ctx: &mut ggez::Context) -> GameResult {
		Ok(())
	}

	fn draw(&mut self, ctx: &mut ggez::Context) -> GameResult {
		let mut canvas = Canvas::from_frame(ctx, Color::BLACK);
		canvas.set_sampler(Sampler::nearest_clamp());
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
