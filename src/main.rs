use gameplay::{LogicalWorld, LogicalWorldTransition};
use ggez::{
	conf::{WindowMode, WindowSetup},
	event::{run, EventHandler},
	glam::IVec2,
	graphics::{Canvas, Color, Image, ImageFormat, Sampler},
	input::keyboard::KeyInput,
	winit::event::VirtualKeyCode,
	Context, ContextBuilder, GameResult,
};
use graphics::GraphicalWorld;
use image::EncodableLayout;

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
		Bunny { hp: i32 },
		Slime { hp: i32, move_token: bool },
	}

	impl Obj {
		fn mass(&self) -> i32 {
			match self {
				Obj::Wall => 10,
				Obj::Slime { .. } => 3,
				Obj::Bunny { .. } => 3,
				_ => 1,
			}
		}

		fn damages(&self) -> i32 {
			match self {
				Obj::Sword => 3,
				Obj::Shield => 0,
				Obj::Slime { .. } => 2,
				_ => 1,
			}
		}

		fn hp(&self) -> Option<i32> {
			match self {
				Obj::Bunny { hp } => Some(*hp),
				Obj::Slime { hp, .. } => Some(*hp),
				_ => None,
			}
		}

		fn take_damage(&mut self, damages: i32) {
			match self {
				Obj::Bunny { hp } => *hp -= damages,
				Obj::Slime { hp, .. } => *hp -= damages,
				_ => {},
			}
		}

		fn is_enemy(&self) -> bool {
			#[allow(clippy::single_match)]
			match self {
				Obj::Slime { .. } => true,
				_ => false,
			}
		}

		fn give_move_token(&mut self) {
			#[allow(clippy::single_match)]
			match self {
				Obj::Slime { move_token, .. } => *move_token = true,
				_ => {},
			}
		}

		fn has_move_token(&self) -> bool {
			match self {
				Obj::Slime { move_token, .. } => *move_token,
				_ => false,
			}
		}

		fn take_move_token(&mut self) -> bool {
			match self {
				Obj::Slime { move_token, .. } => {
					let had_move_token = *move_token;
					*move_token = false;
					had_move_token
				},
				_ => false,
			}
		}
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
			let r = 5;
			for y in (-r)..=r {
				for x in (-r)..=r {
					lw.place_tile(IVec2::new(x, y), Tile::obj(Obj::Wall));
				}
			}
			let r = r - 1;
			for y in (-r)..=r {
				for x in (-r)..=r {
					lw.place_tile(IVec2::new(x, y), Tile::floor());
				}
			}
			lw.place_tile(IVec2::new(-3, 0), Tile::obj(Obj::Sword));
			lw.place_tile(IVec2::new(-2, 0), Tile::obj(Obj::Rock));
			lw.place_tile(IVec2::new(-1, 0), Tile::obj(Obj::Shield));
			lw.place_tile(IVec2::new(0, 0), Tile::obj(Obj::Bunny { hp: 5 }));
			lw.place_tile(
				IVec2::new(2, 0),
				Tile::obj(Obj::Slime { hp: 5, move_token: false }),
			);
			lw.place_tile(
				IVec2::new(3, 1),
				Tile::obj(Obj::Slime { hp: 5, move_token: false }),
			);
			lw.place_tile(
				IVec2::new(3, -1),
				Tile::obj(Obj::Slime { hp: 5, move_token: false }),
			);
			lw.place_tile(IVec2::new(3, 0), Tile::obj(Obj::Wall));
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
				tile.obj.as_ref().is_some_and(|obj| matches!(obj, Obj::Bunny { .. })).then_some(coords)
			})
		}

		pub fn player_move(&self, direction: IVec2) -> LogicalWorldTransition {
			let coords = self.player_coords().unwrap();
			let player_force = 2;
			self.try_to_move(coords, direction, player_force)
		}

		pub fn give_move_token_to_agents(&mut self) {
			for tile in self.grid.values_mut() {
				if let Some(obj) = tile.obj.as_mut() {
					obj.give_move_token();
				}
			}
		}

		pub fn handle_move_for_one_agent(&mut self) -> Option<LogicalWorldTransition> {
			for (coords, tile) in self.grid.iter() {
				if let Some(obj) = tile.obj.as_ref() {
					if obj.has_move_token() {
						let mut res_lw = self.clone();
						res_lw.grid.get_mut(coords).unwrap().obj.as_mut().unwrap().take_move_token();
						return Some(if let Some(direction) = self.ai_decision(*coords) {
							res_lw.try_to_move(*coords, direction, 2)
						} else {
							LogicalWorldTransition { resulting_lw: res_lw, logical_events: vec![] }
						});
					}
				}
			}
			None
		}

		fn ai_decision(&self, agent_coords: IVec2) -> Option<IVec2> {
			// Test simple AI.
			let target_coords = self.player_coords().unwrap();
			let mut direction_opt = if agent_coords.x == target_coords.x {
				if target_coords.y < agent_coords.y {
					Some(IVec2::new(0, -1))
				} else {
					Some(IVec2::new(0, 1))
				}
			} else if agent_coords.y == target_coords.y {
				if target_coords.x < agent_coords.x {
					Some(IVec2::new(-1, 0))
				} else {
					Some(IVec2::new(1, 0))
				}
			} else {
				None
			};
			if let Some(direction) = direction_opt {
				let dst = agent_coords + direction;
				if self
					.grid
					.get(&dst)
					.is_some_and(|tile| tile.obj.as_ref().is_some_and(|obj| obj.is_enemy()))
				{
					direction_opt = None;
				}
			}
			direction_opt
		}

		fn is_hit_killing(&self, weapon_coords: IVec2, direction: IVec2) -> HitAttemptConsequences {
			let weapon_obj = self.grid.get(&weapon_coords).as_ref().unwrap().obj.as_ref().unwrap();
			let target_coords = weapon_coords + direction;
			if let Some(target_obj) = self.grid.get(&target_coords).as_ref().unwrap().obj.as_ref() {
				if let Some(target_hp) = target_obj.hp() {
					let damages = weapon_obj.damages();
					if weapon_obj.damages() >= target_hp {
						HitAttemptConsequences::Kill { damages }
					} else {
						HitAttemptConsequences::NonLethalHit { damages }
					}
				} else {
					HitAttemptConsequences::TargetIsNotHittable
				}
			} else {
				HitAttemptConsequences::ThereIsNoTraget
			}
		}

		/// Returns also the number of objects that do move or that fail to move.
		fn is_move_possible(
			&self,
			pusher_coords: IVec2,
			direction: IVec2,
			force: i32,
		) -> MoveAttemptConsequences {
			let mut coords = pusher_coords;
			let mut remaining_force = force;
			let mut length = 0;
			let mut final_hit = HitAttemptConsequences::ThereIsNoTraget;
			let success = loop {
				coords += direction;
				length += 1;
				if let Some(dst_tile) = self.grid.get(&coords) {
					if let Some(dst_obj) = dst_tile.obj.as_ref() {
						remaining_force -= dst_obj.mass();
						if remaining_force < 0 {
							final_hit = self.is_hit_killing(coords - direction, direction);
							break match final_hit {
								HitAttemptConsequences::Kill { .. } => true,
								HitAttemptConsequences::NonLethalHit { .. } => false,
								HitAttemptConsequences::TargetIsNotHittable => false,
								HitAttemptConsequences::ThereIsNoTraget => unreachable!(),
							};
						}
					} else {
						break true;
					}
				} else {
					break false;
				}
			};
			MoveAttemptConsequences { success, length, final_hit }
		}

		fn try_to_move(
			&self,
			pusher_coords: IVec2,
			direction: IVec2,
			force: i32,
		) -> LogicalWorldTransition {
			let mut res_lw = self.clone();
			let mut logical_events = vec![];
			let MoveAttemptConsequences { success, length, final_hit } =
				self.is_move_possible(pusher_coords, direction, force);
			let mut coords = pusher_coords;
			let mut previous_obj = None;
			for _ in 0..length {
				if success {
					std::mem::swap(
						&mut previous_obj,
						&mut res_lw.grid.get_mut(&coords).unwrap().obj,
					);
					if let Some(obj) = previous_obj.as_ref() {
						logical_events.push(LogicalEvent::Move {
							obj: obj.clone(),
							from: coords,
							to: coords + direction,
						});
					}
				} else {
					let obj = res_lw.grid.get(&coords).as_ref().unwrap().obj.as_ref().unwrap().clone();
					logical_events.push(LogicalEvent::FailToMove {
						obj,
						from: coords,
						to: coords + direction,
					});
				}
				coords += direction;
			}
			if success {
				std::mem::swap(
					&mut previous_obj,
					&mut res_lw.grid.get_mut(&coords).unwrap().obj,
				);
				match final_hit {
					HitAttemptConsequences::Kill { damages } => {
						let target_obj = previous_obj.take().unwrap();
						logical_events.push(LogicalEvent::Killed {
							obj: target_obj,
							at: coords,
							hit_direction: direction,
							damages,
						});
					},
					HitAttemptConsequences::NonLethalHit { .. } => unreachable!(),
					HitAttemptConsequences::TargetIsNotHittable => {},
					HitAttemptConsequences::ThereIsNoTraget => assert!(previous_obj.is_none()),
				}
				assert!(previous_obj.is_none());
			} else {
				match final_hit {
					HitAttemptConsequences::Kill { .. } => unreachable!(),
					HitAttemptConsequences::NonLethalHit { damages } => {
						let target_obj = res_lw.grid.get_mut(&coords).unwrap().obj.as_mut().unwrap();
						target_obj.take_damage(damages);
						logical_events.push(LogicalEvent::Hit {
							obj: target_obj.clone(),
							at: coords,
							hit_direction: direction,
							damages,
						});
					},
					HitAttemptConsequences::TargetIsNotHittable => {},
					HitAttemptConsequences::ThereIsNoTraget => unreachable!(),
				}
			}
			LogicalWorldTransition { resulting_lw: res_lw, logical_events }
		}
	}

	enum HitAttemptConsequences {
		ThereIsNoTraget,
		TargetIsNotHittable,
		NonLethalHit { damages: i32 },
		Kill { damages: i32 },
	}

	struct MoveAttemptConsequences {
		success: bool,
		length: i32,
		final_hit: HitAttemptConsequences,
	}

	#[derive(Clone)]
	pub enum LogicalEvent {
		Move {
			obj: Obj,
			from: IVec2,
			to: IVec2,
		},
		FailToMove {
			obj: Obj,
			from: IVec2,
			to: IVec2,
		},
		Hit {
			obj: Obj,
			at: IVec2,
			hit_direction: IVec2,
			damages: i32,
		},
		Killed {
			obj: Obj,
			at: IVec2,
			hit_direction: IVec2,
			damages: i32,
		},
	}

	#[derive(Clone)]
	pub struct LogicalWorldTransition {
		pub logical_events: Vec<LogicalEvent>,
		pub resulting_lw: LogicalWorld,
	}
}

mod graphics {
	use std::time::{Duration, Instant};

	use ggez::{
		glam::Vec2,
		graphics::{Canvas, Color, DrawParam, Rect},
		Context, GameResult,
	};

	use crate::{
		gameplay::{Ground, LogicalEvent, LogicalWorld, LogicalWorldTransition, Obj},
		SpritesheetStuff,
	};

	enum DepthLayer {
		Floor,
		Obj,
		AnimatedObj,
		TemporaryText,
	}

	impl DepthLayer {
		fn to_z_value(&self) -> i32 {
			match self {
				DepthLayer::Floor => 1,
				DepthLayer::Obj => 2,
				DepthLayer::AnimatedObj => 3,
				DepthLayer::TemporaryText => 4,
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
		Digit(u8),
	}

	impl SpriteFromSheet {
		fn rect_in_spritesheet(&self) -> Rect {
			if let SpriteFromSheet::Digit(digit) = self {
				let x = digit * 4;
				let y = 16;
				return Rect::new(x as f32 / 128.0, y as f32 / 128.0, 3.0 / 128.0, 5.0 / 128.0);
			}
			let (x, y) = match self {
				SpriteFromSheet::Wall => (0, 0),
				SpriteFromSheet::Floor => (0, 1),
				SpriteFromSheet::Sword => (1, 0),
				SpriteFromSheet::Shield => (2, 0),
				SpriteFromSheet::Rock => (3, 0),
				SpriteFromSheet::Bunny => (4, 0),
				SpriteFromSheet::Slime => (5, 0),
				SpriteFromSheet::Digit(_) => unreachable!(),
			};
			Rect::new(
				x as f32 * 8.0 / 128.0,
				y as f32 * 8.0 / 128.0,
				8.0 / 128.0,
				8.0 / 128.0,
			)
		}
	}

	struct TimeInterval {
		start_time: Instant,
		duration: Duration,
	}

	impl TimeInterval {
		fn with_duration(duration: Duration) -> TimeInterval {
			assert!(!duration.is_zero());
			TimeInterval { start_time: Instant::now(), duration }
		}

		fn progress(&self) -> f32 {
			(self.start_time.elapsed().as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
		}
	}

	struct MoveAnimation {
		from: Vec2,
		to: Vec2,
		time_interval: TimeInterval,
	}

	impl MoveAnimation {
		fn new(from: Vec2, to: Vec2) -> MoveAnimation {
			MoveAnimation {
				from,
				to,
				time_interval: TimeInterval::with_duration(Duration::from_secs_f32(0.05)),
			}
		}

		fn current_position(&self) -> Vec2 {
			self.from + self.time_interval.progress() * (self.to - self.from)
		}
	}

	struct FailToMoveAnimation {
		from: Vec2,
		to: Vec2,
		time_interval: TimeInterval,
	}

	impl FailToMoveAnimation {
		fn new(from: Vec2, to: Vec2) -> FailToMoveAnimation {
			FailToMoveAnimation {
				from,
				to,
				time_interval: TimeInterval::with_duration(Duration::from_secs_f32(0.05)),
			}
		}

		fn current_position(&self) -> Vec2 {
			let how_far = 0.2;
			let animation_progress = self.time_interval.progress();
			let to = self.to * how_far + self.from * (1.0 - how_far);
			if animation_progress < 0.5 {
				let forward_prorgess = animation_progress * 2.0;
				self.from + forward_prorgess * (to - self.from)
			} else {
				let backward_prorgess = animation_progress * 2.0 - 1.0;
				to + backward_prorgess * (self.from - to)
			}
		}
	}

	struct HitAnimation {
		time_interval: TimeInterval,
	}

	impl HitAnimation {
		fn new() -> HitAnimation {
			HitAnimation {
				time_interval: TimeInterval::with_duration(Duration::from_secs_f32(0.05)),
			}
		}

		fn current_plain_color(&self) -> Option<Color> {
			(self.time_interval.progress() < 1.0).then_some(Color::RED)
		}
	}

	struct TemporaryTextAnimation {
		from: Vec2,
		to: Vec2,
		color: Color,
		time_interval: TimeInterval,
	}

	impl TemporaryTextAnimation {
		fn new(from: Vec2, to: Vec2, color: Color) -> TemporaryTextAnimation {
			TemporaryTextAnimation {
				from,
				to,
				color,
				time_interval: TimeInterval::with_duration(Duration::from_secs_f32(0.4)),
			}
		}

		fn currently_visible(&self) -> bool {
			self.time_interval.progress() < 1.0
		}

		fn current_position(&self) -> Vec2 {
			self.from + self.time_interval.progress() * (self.to - self.from)
		}

		fn current_plain_color(&self) -> Option<Color> {
			Some(self.color)
		}
	}

	struct DisplayedSprite {
		sprite_from_sheet: SpriteFromSheet,
		center: Vec2,
		depth_layer: DepthLayer,
		move_animation: Option<MoveAnimation>,
		fail_to_move_animation: Option<FailToMoveAnimation>,
		hit_animation: Option<HitAnimation>,
		temporary_text_animation: Option<TemporaryTextAnimation>,
	}

	impl DisplayedSprite {
		fn new(
			sprite_from_sheet: SpriteFromSheet,
			center: Vec2,
			depth_layer: DepthLayer,
			move_animation: Option<MoveAnimation>,
			fail_to_move_animation: Option<FailToMoveAnimation>,
			hit_animation: Option<HitAnimation>,
			temporary_text_animation: Option<TemporaryTextAnimation>,
		) -> DisplayedSprite {
			DisplayedSprite {
				sprite_from_sheet,
				center,
				depth_layer,
				move_animation,
				fail_to_move_animation,
				hit_animation,
				temporary_text_animation,
			}
		}

		fn has_animation(&self) -> bool {
			self.move_animation.as_ref().is_some_and(|anim| anim.time_interval.progress() < 1.0)
				|| self
					.fail_to_move_animation
					.as_ref()
					.is_some_and(|anim| anim.time_interval.progress() < 1.0)
				|| self.hit_animation.as_ref().is_some_and(|anim| anim.time_interval.progress() < 1.0)
				|| self
					.temporary_text_animation
					.as_ref()
					.is_some_and(|anim| anim.time_interval.progress() < 1.0)
		}

		fn visible(&self) -> bool {
			if let Some(temporary_text_animation) = self.temporary_text_animation.as_ref() {
				temporary_text_animation.currently_visible()
			} else {
				true
			}
		}

		fn center(&self) -> Vec2 {
			if let Some(move_animation) = self.move_animation.as_ref() {
				move_animation.current_position()
			} else if let Some(fail_to_move_animation) = self.fail_to_move_animation.as_ref() {
				fail_to_move_animation.current_position()
			} else if let Some(temporary_text_animation) = self.temporary_text_animation.as_ref() {
				temporary_text_animation.current_position()
			} else {
				self.center
			}
		}

		fn plain_color(&self) -> Option<Color> {
			if let Some(hit_animation) = self.hit_animation.as_ref() {
				hit_animation.current_plain_color()
			} else if let Some(temporary_text_animation) = self.temporary_text_animation.as_ref() {
				temporary_text_animation.current_plain_color()
			} else {
				None
			}
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
			let lw_trans = LogicalWorldTransition { resulting_lw: lw.clone(), logical_events: vec![] };
			GraphicalWorld::from_logical_world_transition(&lw_trans)
		}

		pub fn has_animation(&self) -> bool {
			self.sprites.iter().any(|sprite| sprite.has_animation())
		}

		pub fn from_logical_world_transition(lw_trans: &LogicalWorldTransition) -> GraphicalWorld {
			let mut gw = GraphicalWorld::new();
			for (coords, tile) in lw_trans.resulting_lw.tiles() {
				if matches!(tile.ground, Ground::Floor) {
					gw.add_sprite(DisplayedSprite::new(
						SpriteFromSheet::Floor,
						coords.as_vec2(),
						DepthLayer::Floor,
						None,
						None,
						None,
						None,
					));
				}
				if let Some(obj) = tile.obj.as_ref() {
					let sprite_from_sheet = match obj {
						Obj::Wall => SpriteFromSheet::Wall,
						Obj::Sword => SpriteFromSheet::Sword,
						Obj::Shield => SpriteFromSheet::Shield,
						Obj::Rock => SpriteFromSheet::Rock,
						Obj::Bunny { .. } => SpriteFromSheet::Bunny,
						Obj::Slime { .. } => SpriteFromSheet::Slime,
					};
					let move_animation =
						lw_trans.logical_events.iter().find_map(|logical_event| match logical_event {
							LogicalEvent::Move { from, to, .. } if *to == coords => {
								Some(MoveAnimation::new(from.as_vec2(), to.as_vec2()))
							},
							_ => None,
						});
					let fail_to_move_animation =
						lw_trans.logical_events.iter().find_map(|logical_event| match logical_event {
							LogicalEvent::FailToMove { from, to, .. } if *from == coords => {
								Some(FailToMoveAnimation::new(from.as_vec2(), to.as_vec2()))
							},
							_ => None,
						});
					let hit_animation =
						lw_trans.logical_events.iter().find_map(|logical_event| match logical_event {
							LogicalEvent::Hit { at, .. } if *at == coords => Some(HitAnimation::new()),
							_ => None,
						});
					let depth_layer = if move_animation.is_some() || fail_to_move_animation.is_some() {
						DepthLayer::AnimatedObj
					} else {
						DepthLayer::Obj
					};
					gw.add_sprite(DisplayedSprite::new(
						sprite_from_sheet,
						coords.as_vec2(),
						depth_layer,
						move_animation,
						fail_to_move_animation,
						hit_animation,
						None,
					));
				}
			}
			for logical_event in lw_trans.logical_events.iter() {
				match logical_event {
					LogicalEvent::Killed { at, damages, .. } | LogicalEvent::Hit { at, damages, .. } => {
						gw.add_sprite(DisplayedSprite::new(
							SpriteFromSheet::Digit(*damages as u8),
							at.as_vec2(),
							DepthLayer::TemporaryText,
							None,
							None,
							None,
							Some(TemporaryTextAnimation::new(
								at.as_vec2(),
								at.as_vec2() + Vec2::new(0.0, -1.5),
								Color::RED,
							)),
						));
					},
					_ => {},
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
			spritesheet_stuff: &SpritesheetStuff,
		) -> GameResult {
			let sprite_size_px = 8.0 * 7.0;
			for sprite in self.sprites.iter() {
				if !sprite.visible() {
					continue;
				}
				let center = sprite.center();
				let top_left = center * sprite_size_px - Vec2::new(1.0, 1.0) * sprite_size_px / 2.0;
				let top_left = top_left + Vec2::new(400.0, 400.0);
				let plain_color = sprite.plain_color();
				let (spritesheet, color) = if let Some(color) = plain_color {
					(&spritesheet_stuff.spritesheet_white, color)
				} else {
					(&spritesheet_stuff.spritesheet, Color::WHITE)
				};
				canvas.draw(
					spritesheet,
					DrawParam::default()
						.dest(top_left)
						.offset(Vec2::new(0.5, 0.5))
						.scale(Vec2::new(1.0, 1.0) * sprite_size_px / 8.0)
						.src(sprite.sprite_from_sheet.rect_in_spritesheet())
						.z(sprite.depth_layer.to_z_value())
						.color(color),
				);
			}
			Ok(())
		}
	}
}

struct SpritesheetStuff {
	spritesheet: Image,
	/// Used as a mask to multiply it by a color for like hit effect red blinking.
	spritesheet_white: Image,
}

impl SpritesheetStuff {
	fn new(ctx: &mut Context) -> GameResult<SpritesheetStuff> {
		let mut image = image::load_from_memory(include_bytes!("../assets/spritesheet.png")).unwrap();
		let spritesheet = Image::from_pixels(
			&ctx.gfx,
			image.as_rgba8().unwrap().as_bytes(),
			ImageFormat::Rgba8UnormSrgb,
			image.width(),
			image.height(),
		);

		image.as_mut_rgba8().unwrap().pixels_mut().for_each(|pixel| {
			if pixel.0[3] != 0 {
				pixel.0[0] = 255;
				pixel.0[1] = 255;
				pixel.0[2] = 255;
			}
		});
		let spritesheet_white = Image::from_pixels(
			&ctx.gfx,
			image.as_rgba8().unwrap().as_bytes(),
			ImageFormat::Rgba8UnormSrgb,
			image.width(),
			image.height(),
		);

		Ok(SpritesheetStuff { spritesheet, spritesheet_white })
	}
}

enum Phase {
	WaitingForPlayerToMakeAMove,
	WaitingForAnimationsToFinish(Vec<LogicalWorldTransition>),
}

struct Game {
	current_lw: LogicalWorld,
	phase: Phase,
	gw: GraphicalWorld,
	spritesheet_stuff: SpritesheetStuff,
}

impl Game {
	fn new(ctx: &mut Context) -> GameResult<Game> {
		let lw = LogicalWorld::new_test();
		let gw = GraphicalWorld::from_logical_world(&lw);
		let spritesheet_stuff = SpritesheetStuff::new(ctx)?;
		let phase = Phase::WaitingForPlayerToMakeAMove;
		Ok(Game { current_lw: lw, phase, gw, spritesheet_stuff })
	}

	fn player_move(&mut self, direction: IVec2) {
		if matches!(self.phase, Phase::WaitingForPlayerToMakeAMove) {
			let mut lw_trans = self.current_lw.player_move(direction);
			self.gw = GraphicalWorld::from_logical_world_transition(&lw_trans);
			self.current_lw = lw_trans.resulting_lw.clone();

			// Play all the moves of everyting that is not a player up until the player's next turn.
			lw_trans.resulting_lw.give_move_token_to_agents();
			let mut transitions = vec![];
			while let Some(next_lw_trans) = lw_trans.resulting_lw.handle_move_for_one_agent() {
				transitions.push(next_lw_trans.clone());
				lw_trans = next_lw_trans;
			}
			self.phase = Phase::WaitingForAnimationsToFinish(transitions);
		}
	}
}

impl EventHandler for Game {
	fn update(&mut self, _ctx: &mut Context) -> GameResult {
		let no_more_animations = !self.gw.has_animation();
		if no_more_animations {
			if let Phase::WaitingForAnimationsToFinish(next_tranitions) = &mut self.phase {
				if next_tranitions.is_empty() {
					self.phase = Phase::WaitingForPlayerToMakeAMove;
				} else {
					let lw_trans = next_tranitions.remove(0);
					self.gw = GraphicalWorld::from_logical_world_transition(&lw_trans);
					self.current_lw = lw_trans.resulting_lw.clone();
				}
			}
		}
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
		self.gw.draw(ctx, &mut canvas, &self.spritesheet_stuff)?;
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
