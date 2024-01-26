use gameplay::LogicalWorld;
use ggez::{
	conf::{WindowMode, WindowSetup},
	event::{run, EventHandler},
	glam::IVec2,
	graphics::{Canvas, Color, Image, Sampler},
	input::keyboard::KeyInput,
	winit::event::VirtualKeyCode,
	Context, ContextBuilder, GameResult,
};
use graphics::GraphicalWorld;

mod gameplay {
	use std::collections::HashMap;

	use ggez::glam::IVec2;

	#[derive(Clone)]
	pub enum Ground {
		Floor,
		// TODO: Hole
	}

	#[derive(Clone)]
	pub enum Obj {
		Wall,
		Sword,
		Shield,
		Rock,
		Bunny,
		Slime { hp: i32 },
	}

	#[derive(Clone)]
	pub struct Tile {
		pub ground: Ground,
		pub obj: Option<Obj>,
		pub visible: bool,
	}

	impl Tile {
		fn floor() -> Tile {
			Tile { ground: Ground::Floor, obj: None, visible: false }
		}
		fn obj(obj: Obj) -> Tile {
			Tile { ground: Ground::Floor, obj: Some(obj), visible: false }
		}
	}

	#[derive(Clone)]
	pub struct LogicalWorld {
		grid: HashMap<IVec2, Tile>,
	}

	impl LogicalWorld {
		pub fn new_empty() -> LogicalWorld {
			LogicalWorld { grid: HashMap::new() }
		}

		pub fn new_test() -> LogicalWorld {
			let mut lw = LogicalWorld::new_empty();
			lw.place_tile(IVec2::new(-3, 0), Tile::obj(Obj::Sword));
			lw.place_tile(IVec2::new(-2, 0), Tile::obj(Obj::Rock));
			lw.place_tile(IVec2::new(-1, 0), Tile::obj(Obj::Shield));
			lw.place_tile(IVec2::new(0, 0), Tile::obj(Obj::Bunny));
			lw.place_tile(IVec2::new(1, 0), Tile::floor());
			lw.place_tile(IVec2::new(2, 0), Tile::obj(Obj::Slime { hp: 3 }));
			lw.place_tile(IVec2::new(3, 0), Tile::obj(Obj::Wall));
			lw.place_tile(IVec2::new(0, 1), Tile::floor());
			lw.place_tile(IVec2::new(1, 1), Tile::floor());
			lw
		}

		fn place_tile(&mut self, coords: IVec2, tile: Tile) {
			self.grid.insert(coords, tile);
		}

		pub fn tiles(&self) -> impl Iterator<Item = (IVec2, &Tile)> {
			self.grid.iter().map(|(&coords, tile)| (coords, tile))
		}

		fn player_coords(&self) -> Option<IVec2> {
			self.grid.iter().find_map(|(&coords, tile)| {
				tile.obj.as_ref().is_some_and(|obj| matches!(obj, Obj::Bunny)).then_some(coords)
			})
		}

		pub fn player_move(&self, direction: IVec2) -> LogicalWorldTransition {
			let mut res_lw = self.clone();
			let src = self.player_coords().unwrap();
			let dst = src + direction;
			let player_obj = res_lw.grid.get_mut(&src).unwrap().obj.take().unwrap();
			res_lw.grid.get_mut(&dst).unwrap().obj = Some(player_obj.clone());
			let logical_events = vec![LogicalEvent::Move { obj: player_obj, from: src, to: dst }];
			LogicalWorldTransition { resulting_lw: res_lw, logical_events }
		}
	}

	pub enum LogicalEvent {
		Move { obj: Obj, from: IVec2, to: IVec2 },
	}

	pub struct LogicalWorldTransition {
		pub logical_events: Vec<LogicalEvent>,
		pub resulting_lw: LogicalWorld,
	}
}

mod graphics {
	use ggez::{
		glam::Vec2,
		graphics::{Canvas, Color, DrawParam, Image, Rect},
		Context, GameResult,
	};

	use crate::gameplay::{Ground, LogicalWorld, Obj};

	enum DepthLayer {
		Floor,
		Obj,
	}

	impl DepthLayer {
		fn to_z_value(&self) -> i32 {
			match self {
				DepthLayer::Floor => 1,
				DepthLayer::Obj => 2,
			}
		}
	}

	enum SpriteFromSheet {
		Wall,
		Floor,
		Sword,
		Shield,
		Rock,
		Bunny,
		Slime,
	}

	impl SpriteFromSheet {
		fn rect_in_spritesheet(&self) -> Rect {
			let (x, y) = match self {
				SpriteFromSheet::Wall => (0, 0),
				SpriteFromSheet::Floor => (0, 1),
				SpriteFromSheet::Sword => (1, 0),
				SpriteFromSheet::Shield => (2, 0),
				SpriteFromSheet::Rock => (3, 0),
				SpriteFromSheet::Bunny => (4, 0),
				SpriteFromSheet::Slime => (5, 0),
			};
			Rect::new(
				x as f32 * 8.0 / 128.0,
				y as f32 * 8.0 / 128.0,
				8.0 / 128.0,
				8.0 / 128.0,
			)
		}
	}

	struct DisplayedSprite {
		sprite_from_sheet: SpriteFromSheet,
		center: Vec2,
		depth_layer: DepthLayer,
		// TODO: add effects here
	}

	impl DisplayedSprite {
		fn new(
			sprite_from_sheet: SpriteFromSheet,
			center: Vec2,
			depth_layer: DepthLayer,
		) -> DisplayedSprite {
			DisplayedSprite { sprite_from_sheet, center, depth_layer }
		}
	}

	pub struct GraphicalWorld {
		sprites: Vec<DisplayedSprite>,
	}

	impl GraphicalWorld {
		pub fn new() -> GraphicalWorld {
			GraphicalWorld { sprites: vec![] }
		}

		pub fn from_logical_world(lw: &LogicalWorld) -> GraphicalWorld {
			let mut gw = GraphicalWorld::new();
			for (coords, tile) in lw.tiles() {
				if matches!(tile.ground, Ground::Floor) {
					gw.add_sprite(DisplayedSprite::new(
						SpriteFromSheet::Floor,
						coords.as_vec2(),
						DepthLayer::Floor,
					));
				}
				if let Some(obj) = tile.obj.as_ref() {
					let sprite_from_sheet = match obj {
						Obj::Wall => SpriteFromSheet::Wall,
						Obj::Sword => SpriteFromSheet::Sword,
						Obj::Shield => SpriteFromSheet::Shield,
						Obj::Rock => SpriteFromSheet::Rock,
						Obj::Bunny => SpriteFromSheet::Bunny,
						Obj::Slime { .. } => SpriteFromSheet::Slime,
					};
					gw.add_sprite(DisplayedSprite::new(
						sprite_from_sheet,
						coords.as_vec2(),
						DepthLayer::Obj,
					));
				}
			}
			gw
		}

		fn add_sprite(&mut self, displayed_sprite: DisplayedSprite) {
			self.sprites.push(displayed_sprite);
		}

		pub fn draw(
			&self,
			_ctx: &mut Context,
			canvas: &mut Canvas,
			spritesheet: &Image,
		) -> GameResult {
			let sprite_size_px = 8.0 * 7.0;
			for sprite in self.sprites.iter() {
				let top_left =
					sprite.center * sprite_size_px - Vec2::new(1.0, 1.0) * sprite_size_px / 2.0;
				let top_left = top_left + Vec2::new(400.0, 400.0);
				canvas.draw(
					spritesheet,
					DrawParam::default()
						.dest(top_left)
						.offset(Vec2::new(0.5, 0.5))
						.scale(Vec2::new(1.0, 1.0) * sprite_size_px / 8.0)
						.src(sprite.sprite_from_sheet.rect_in_spritesheet())
						.z(sprite.depth_layer.to_z_value())
						.color(Color::WHITE),
				);
			}
			Ok(())
		}
	}
}

struct Game {
	current_lw: LogicalWorld,
	gw: GraphicalWorld,
	spritesheet: Image,
}

impl Game {
	fn new(ctx: &mut Context) -> GameResult<Game> {
		let lw = LogicalWorld::new_test();
		let gw = GraphicalWorld::from_logical_world(&lw);
		let spritesheet = Image::from_bytes(ctx, include_bytes!("../assets/spritesheet.png"))?;
		Ok(Game { current_lw: lw, gw, spritesheet })
	}

	fn player_move(&mut self, direction: IVec2) {
		let transition = self.current_lw.player_move(direction);
		self.gw = GraphicalWorld::from_logical_world(&transition.resulting_lw);
		self.current_lw = transition.resulting_lw;
	}
}

impl EventHandler for Game {
	fn update(&mut self, _ctx: &mut Context) -> GameResult {
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
		self.gw.draw(ctx, &mut canvas, &self.spritesheet)?;
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
