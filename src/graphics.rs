//! The rendering of the logical states and transitions between them happens here.
//!
//! When a state or a state transition is rendered, it is rendered once into a graphical world
//! made of animated sprites.
//! The animations then play out as the graphical world is itself drawn to the screen each frame.
//! These are like two levels of rendering, the first creates sprites and defines animations,
//! and the second draws the sprites and plays the animations.

use std::time::{Duration, Instant};

use ggez::{
	glam::Vec2,
	graphics::{Canvas, Color, DrawParam},
	Context, GameResult,
};

use crate::{
	gameplay::{Ground, LogicalEvent, LogicalTransition, LogicalWorld, Obj},
	spritesheet::{SpriteFromSheet, SpritesheetStuff},
};

enum DepthLayer {
	Floor,
	Obj,
	AnimatedObj,
	TemporaryText,
	Interface,
}

impl DepthLayer {
	fn to_z_value(&self) -> i32 {
		// Higer is closer to foreground, lower is closer to background.
		match self {
			DepthLayer::Floor => 1,
			DepthLayer::Obj => 2,
			DepthLayer::AnimatedObj => 3,
			DepthLayer::TemporaryText => 4,
			DepthLayer::Interface => 5,
		}
	}
}

/// An instance of a sprite that has a position, depth layer and animations.
struct DisplayedSprite {
	sprite_from_sheet: SpriteFromSheet,
	center: Vec2,
	depth_layer: DepthLayer,
	/// Is it in the world (and should move with the camera) or not (like a piece of interface)?
	in_world: bool,
	plain_color: Option<Color>,
	height_for_scale: Option<f32>,
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
		in_world: bool,
		plain_color: Option<Color>,
		height_for_scale: Option<f32>,
		animations: Animations,
	) -> DisplayedSprite {
		let Animations {
			move_animation,
			fail_to_move_animation,
			hit_animation,
			temporary_text_animation,
		} = animations;
		DisplayedSprite {
			sprite_from_sheet,
			center,
			depth_layer,
			in_world,
			plain_color,
			height_for_scale,
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
		if let Some(move_animation) = self.move_animation.as_ref() {
			move_animation.currently_visible()
		} else if let Some(temporary_text_animation) = self.temporary_text_animation.as_ref() {
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
		.or(self.plain_color)
	}
}

fn obj_to_sprite(obj: &Obj) -> SpriteFromSheet {
	match obj {
		Obj::Wall => SpriteFromSheet::Wall,
		Obj::Sword => SpriteFromSheet::Sword,
		Obj::Shield => SpriteFromSheet::Shield,
		Obj::Pickaxe => SpriteFromSheet::Pickaxe,
		Obj::Rock => SpriteFromSheet::Rock,
		Obj::Door => SpriteFromSheet::Door,
		Obj::Key => SpriteFromSheet::Key,
		Obj::Rope => SpriteFromSheet::Rope,
		Obj::Exit => SpriteFromSheet::Exit,
		Obj::VisionGem => SpriteFromSheet::VisionGem,
		Obj::Heart => SpriteFromSheet::Heart,
		Obj::RedoHeart => SpriteFromSheet::RedoHeart,
		Obj::Bunny { .. } => SpriteFromSheet::Bunny,
		Obj::Slime { .. } => SpriteFromSheet::Slime,
		Obj::Shroomer { .. } => SpriteFromSheet::Shroomer,
		Obj::Shroom => SpriteFromSheet::Shroom,
	}
}

/// The world, as a set of animated sprites, to be displayed.
/// It represents a logical world or even a transition to a logical world,
/// but the logical nature of things is lost to sprites, it is a render in a sense.
pub struct GraphicalWorld {
	sprites: Vec<DisplayedSprite>,
	pub info_for_camera: InfoForCamera,
}

impl GraphicalWorld {
	pub fn new() -> GraphicalWorld {
		GraphicalWorld { sprites: vec![], info_for_camera: InfoForCamera::new() }
	}

	pub fn from_logical_world(lw: &LogicalWorld) -> GraphicalWorld {
		let transition = LogicalTransition { resulting_lw: lw.clone(), logical_events: vec![] }
			.updated_visibility();
		GraphicalWorld::from_logical_world_transition(&transition)
	}

	/// Are animations still playing, or are they all finished?
	pub fn has_animation(&self) -> bool {
		self.sprites.iter().any(|sprite| sprite.has_animation())
	}

	/// Renders the transition to a logical world as a graphical world,
	/// using animations to convey the transition, and making sure that as animations end
	/// the remaining representation depicts the logical world that results from the transition.
	pub fn from_logical_world_transition(transition: &LogicalTransition) -> GraphicalWorld {
		let mut gw = GraphicalWorld::new();
		let mut bunny_copy = None;
		// We iterate over all the tiles, creating sprites to represent their content.
		for (coords, tile) in transition.resulting_lw.tiles() {
			if !tile.visible {
				continue;
			}
			// Ground.
			if matches!(tile.ground, Ground::Floor) {
				gw.add_sprite(DisplayedSprite::new(
					SpriteFromSheet::Floor,
					coords.as_vec2(),
					DepthLayer::Floor,
					true,
					None,
					None,
					Animations::new(None, None, None, None),
				));
			}
			// Object.
			if let Some(obj) = tile.obj.as_ref() {
				let sprite_from_sheet = obj_to_sprite(obj);
				if matches!(obj, Obj::Bunny { .. }) {
					bunny_copy = Some(obj);
					gw.info_for_camera.player_position = Some(coords.as_vec2());
				}
				// If the object is mentioned by a logical event of the transition,
				// then it may be animated to represent that event happening.
				let move_animation =
					transition.logical_events.iter().find_map(|logical_event| match logical_event {
						LogicalEvent::Move { from, to, .. } if *to == coords => {
							Some(MoveAnimation::new(from.as_vec2(), to.as_vec2()))
						},
						_ => None,
					});
				let fail_to_move_animation =
					transition.logical_events.iter().find_map(|logical_event| match logical_event {
						LogicalEvent::FailToMove { from, to, .. } if *from == coords => {
							Some(FailToMoveAnimation::new(from.as_vec2(), to.as_vec2()))
						},
						_ => None,
					});
				let hit_animation = {
					transition.logical_events.iter().find_map(|logical_event| match logical_event {
						LogicalEvent::Hit { at, .. } if *at == coords => Some(HitAnimation::new()),
						_ => None,
					})
					// Note that the damage number that appears and floats away is handled after.
				};
				let depth_layer = if move_animation.is_some() || fail_to_move_animation.is_some() {
					DepthLayer::AnimatedObj
				} else {
					DepthLayer::Obj
				};
				gw.add_sprite(DisplayedSprite::new(
					sprite_from_sheet,
					coords.as_vec2(),
					depth_layer,
					true,
					None,
					None,
					Animations::new(move_animation, fail_to_move_animation, hit_animation, None),
				));
			}
		}
		// Some sprites represent events which are not exactly representations of tiles.
		for logical_event in transition.logical_events.iter() {
			match logical_event {
				LogicalEvent::Killed { at, damages, .. } | LogicalEvent::Hit { at, damages, .. } => {
					// When damages are dealt, a damage number shall appear and float away.
					if transition.resulting_lw.tile(*at).is_some_and(|tile| tile.visible) {
						gw.add_sprite(DisplayedSprite::new(
							SpriteFromSheet::Digit(*damages as u8),
							at.as_vec2(),
							DepthLayer::TemporaryText,
							true,
							None,
							None,
							Animations::new(
								None,
								None,
								None,
								Some(TemporaryTextAnimation::new(
									at.as_vec2() + Vec2::new(0.0, -0.5),
									at.as_vec2() + Vec2::new(0.0, -1.5),
									Color::RED,
								)),
							),
						));
					}
				},
				LogicalEvent::Exit { obj, from, to } => {
					if transition.resulting_lw.tile(*from).is_some_and(|tile| tile.visible) {
						let sprite_from_sheet = obj_to_sprite(obj);
						gw.add_sprite(DisplayedSprite::new(
							sprite_from_sheet,
							to.as_vec2(),
							DepthLayer::AnimatedObj,
							true,
							None,
							None,
							Animations::new(
								Some(MoveAnimation::new_disappear_after(
									from.as_vec2(),
									to.as_vec2(),
								)),
								None,
								None,
								None,
							),
						));
					}
				},
				LogicalEvent::DoorOpenedWithKey { key_obj, door_obj, from, to } => {
					if transition.resulting_lw.tile(*from).is_some_and(|tile| tile.visible) {
						gw.add_sprite(DisplayedSprite::new(
							obj_to_sprite(key_obj),
							to.as_vec2(),
							DepthLayer::AnimatedObj,
							true,
							None,
							None,
							Animations::new(
								Some(MoveAnimation::new_disappear_after(
									from.as_vec2(),
									to.as_vec2(),
								)),
								None,
								None,
								None,
							),
						));
						gw.add_sprite(DisplayedSprite::new(
							obj_to_sprite(door_obj),
							to.as_vec2(),
							DepthLayer::AnimatedObj,
							true,
							None,
							None,
							Animations::new(
								Some(MoveAnimation::new_disappear_after(
									to.as_vec2(),
									to.as_vec2(),
								)),
								None,
								None,
								None,
							),
						));
					}
				},
				_ => {},
			}
		}

		// Interface.
		let interface_scale = 5.0;
		let char_height = 5.0 * interface_scale;
		let char_width = 3.0 * interface_scale;
		let space_width = 1.0 * interface_scale;
		let heart_width = 7.0 * interface_scale;
		let heart_height = 8.0 * interface_scale;
		let heart_rescale = 5.0 / 6.0;
		let heart_y_offset = -1.0 * interface_scale;
		let mut add_char_sprite =
			|sprite_from_sheet: SpriteFromSheet, center: Vec2, height: f32, white: bool| {
				gw.add_sprite(DisplayedSprite::new(
					sprite_from_sheet,
					center,
					DepthLayer::Interface,
					false,
					white.then_some(Color::WHITE),
					Some(height),
					Animations::new(None, None, None, None),
				));
			};
		let ui_x = 15.0;

		// Redo count.
		let base_y = 20.0;
		add_char_sprite(
			SpriteFromSheet::RedoHeart,
			Vec2::new(ui_x, base_y + heart_y_offset)
				+ Vec2::new(heart_width, heart_height) * heart_rescale / 2.0,
			heart_height * heart_rescale,
			false,
		);
		add_char_sprite(
			SpriteFromSheet::Digit(transition.resulting_lw.redo_count as u8),
			Vec2::new(ui_x, base_y)
				+ Vec2::new(char_width, char_height) / 2.0
				+ Vec2::new(heart_width + space_width, 0.0),
			char_height,
			true,
		);
		add_char_sprite(
			SpriteFromSheet::Slash,
			Vec2::new(ui_x, base_y)
				+ Vec2::new(char_width, char_height) / 2.0
				+ Vec2::new(heart_width + char_width + space_width * 2.0, 0.0),
			char_height,
			true,
		);
		add_char_sprite(
			SpriteFromSheet::Digit(transition.resulting_lw.max_redo_count as u8),
			Vec2::new(ui_x, base_y)
				+ Vec2::new(char_width, char_height) / 2.0
				+ Vec2::new(heart_width + char_width * 2.0 + space_width * 3.0, 0.0),
			char_height,
			true,
		);

		// HP count.
		if let Some(Obj::Bunny { hp, max_hp }) = bunny_copy {
			let base_y = 60.0;
			add_char_sprite(
				SpriteFromSheet::Heart,
				Vec2::new(ui_x, base_y + heart_y_offset)
					+ Vec2::new(heart_width, heart_height) * heart_rescale / 2.0,
				heart_height * heart_rescale,
				false,
			);
			add_char_sprite(
				SpriteFromSheet::Digit(*hp as u8),
				Vec2::new(ui_x, base_y)
					+ Vec2::new(char_width, char_height) / 2.0
					+ Vec2::new(heart_width + space_width, 0.0),
				char_height,
				true,
			);
			add_char_sprite(
				SpriteFromSheet::Slash,
				Vec2::new(ui_x, base_y)
					+ Vec2::new(char_width, char_height) / 2.0
					+ Vec2::new(heart_width + char_width + space_width * 2.0, 0.0),
				char_height,
				true,
			);
			add_char_sprite(
				SpriteFromSheet::Digit(*max_hp as u8),
				Vec2::new(ui_x, base_y)
					+ Vec2::new(char_width, char_height) / 2.0
					+ Vec2::new(heart_width + char_width * 2.0 + space_width * 3.0, 0.0),
				char_height,
				true,
			);
		}
		gw
	}

	fn add_sprite(&mut self, displayed_sprite: DisplayedSprite) {
		self.sprites.push(displayed_sprite);
	}

	/// Render the rendering!
	pub fn draw(
		&self,
		_ctx: &mut Context,
		canvas: &mut Canvas,
		spritesheet_stuff: &SpritesheetStuff,
		camera: &Camera,
	) -> GameResult {
		let tile_size_px = camera.tile_size_px();
		let camera_pos = (camera.current_position * tile_size_px).as_ivec2().as_vec2() / tile_size_px;
		for sprite in self.sprites.iter() {
			if !sprite.visible() {
				continue;
			}
			let center = sprite.center();
			let dest = if sprite.in_world {
				(center - camera_pos) * tile_size_px + Vec2::new(400.0, 400.0)
			} else {
				center
			};
			let plain_color = sprite.plain_color();
			let (spritesheet, color) = if let Some(color) = plain_color {
				// A plain color shall be multiplied to the sprite, but we want all the sprite
				// to be exactly of that *plain* color, so we choose a variant of the sprite that
				// is all white. We find it in the spritesheet that was painted in white.
				(&spritesheet_stuff.spritesheet_white, color)
			} else {
				(&spritesheet_stuff.spritesheet, Color::WHITE)
			};
			let rect_in_spritesheet = {
				let mut rect = sprite.sprite_from_sheet.rect_in_spritesheet();
				// Acceptable hack imho: Reduce a tiny bit the rect in the spritesheet,
				// less than what would be necessary to see a difference,
				// but enough so that edges of the rect are not ambiguously touching adjacent sprites.
				// Not doing so leads to edges of adjacent sprites being sometime visible for a frame
				// where they are not wanted, which is bad.
				let margin = 0.03 / 128.0;
				rect.x += margin;
				rect.y += margin;
				rect.w -= margin * 2.0;
				rect.h -= margin * 2.0;
				rect
			};
			let height_for_scale = sprite.height_for_scale.unwrap_or(tile_size_px);
			canvas.draw(
				spritesheet,
				DrawParam::default()
					.dest(dest)
					.offset(Vec2::new(0.5, 0.5))
					.scale(Vec2::new(1.0, 1.0) * height_for_scale / (rect_in_spritesheet.h * 128.0))
					.src(rect_in_spritesheet)
					.z(sprite.depth_layer.to_z_value())
					.color(color),
			);
		}
		Ok(())
	}
}

/// An animation plays during some time interval, and progresses during said interval.
struct TimeInterval {
	start_time: Instant,
	duration: Duration,
}

impl TimeInterval {
	/// Starts now.
	fn with_duration(duration: Duration) -> TimeInterval {
		assert!(!duration.is_zero());
		TimeInterval { start_time: Instant::now(), duration }
	}

	/// Zero before and at staring time,
	/// progresses from zero to one linearly during the time interval
	/// and stays at one at and after the end.
	fn progress(&self) -> f32 {
		(self.start_time.elapsed().as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
	}
}

/// A sprites move linearly and then remain at its target position.
///
/// Can be used on the sprites of objects that move and are pushed.
struct MoveAnimation {
	from: Vec2,
	to: Vec2,
	time_interval: TimeInterval,
	disappear_after: bool,
}

impl MoveAnimation {
	fn new(from: Vec2, to: Vec2) -> MoveAnimation {
		MoveAnimation {
			from,
			to,
			time_interval: TimeInterval::with_duration(Duration::from_secs_f32(0.05)),
			disappear_after: false,
		}
	}

	fn new_disappear_after(from: Vec2, to: Vec2) -> MoveAnimation {
		MoveAnimation {
			from,
			to,
			time_interval: TimeInterval::with_duration(Duration::from_secs_f32(0.05)),
			disappear_after: true,
		}
	}

	fn currently_visible(&self) -> bool {
		!(self.disappear_after && self.time_interval.progress() >= 1.0)
	}

	fn current_position(&self) -> Vec2 {
		self.from + self.time_interval.progress() * (self.to - self.from)
	}
}

/// A sprites begins to move to its target position, but along the way it changes course
/// to go back to its starting position, and remains there.
///
/// Can be used on the sprites of objects that fail to push.
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
		// A factor of how far long the way does the course changes
		// to target the starting position.
		let how_far = 0.3;

		let animation_progress = self.time_interval.progress();
		// The real target position of the first half of the animation, the point
		// at which the course changes.
		let to = self.to * how_far + self.from * (1.0 - how_far);
		if animation_progress < 0.5 {
			let forward_prorgess = animation_progress * 2.0;
			// In the first half of the animation, it is just a move to the real target position.
			self.from + forward_prorgess * (to - self.from)
		} else {
			let backward_prorgess = animation_progress * 2.0 - 1.0;
			// In the second half, the strating and target positions are swapped.
			to + backward_prorgess * (self.from - to)
		}
	}
}

/// All the sprite appears plain red for the specified duration.
///
/// This represents being hit and is used on the sprites of objects
/// that take a non-lethal hit.
struct HitAnimation {
	time_interval: TimeInterval,
}

impl HitAnimation {
	fn new() -> HitAnimation {
		HitAnimation {
			time_interval: TimeInterval::with_duration(Duration::from_secs_f32(0.15)),
		}
	}

	fn current_plain_color(&self) -> Option<Color> {
		(self.time_interval.progress() < 1.0).then_some(Color::RED)
	}
}

/// The sprite moves from and to the specified positions,
/// appearing the specified plain color, and then vanishes at the end.
///
/// This is used on temporary text that appears on the grid, like for example the damage
/// numbers of hits that are colored digits going up and then disappearing.
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
			time_interval: TimeInterval::with_duration(Duration::from_secs_f32(0.2)),
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

struct Animations {
	move_animation: Option<MoveAnimation>,
	fail_to_move_animation: Option<FailToMoveAnimation>,
	hit_animation: Option<HitAnimation>,
	temporary_text_animation: Option<TemporaryTextAnimation>,
}

impl Animations {
	fn new(
		move_animation: Option<MoveAnimation>,
		fail_to_move_animation: Option<FailToMoveAnimation>,
		hit_animation: Option<HitAnimation>,
		temporary_text_animation: Option<TemporaryTextAnimation>,
	) -> Animations {
		Animations {
			move_animation,
			fail_to_move_animation,
			hit_animation,
			temporary_text_animation,
		}
	}
}

/// Info about the logical or graphical world that can help the camera set its target.
pub struct InfoForCamera {
	player_position: Option<Vec2>,
}

impl InfoForCamera {
	fn new() -> InfoForCamera {
		InfoForCamera { player_position: None }
	}
}

/// Points to a position in the world that ends up displayed at the center of the window.
/// When the target moves (even abruptly), the camera follows smoothly.
/// Also hold the zoom level.
pub struct Camera {
	target_position: Vec2,
	current_position: Vec2,
	/// Some number that represents how fast the camera moves to follow the target.
	speed: f32,
	/// A pixel in the spritesheet will be scaled up by this factor.
	sprite_px_scaled_to_how_many_screen_px: i32,
}

impl Camera {
	pub fn new() -> Camera {
		Camera {
			target_position: Vec2::new(0.0, 0.0),
			current_position: Vec2::new(0.0, 0.0),
			speed: 3.0,
			sprite_px_scaled_to_how_many_screen_px: 7,
		}
	}

	/// How long an edge of a tile should appear on the screen, measured in screen pixels.
	fn tile_size_px(&self) -> f32 {
		self.sprite_px_scaled_to_how_many_screen_px as f32 * 8.0
	}

	/// Make the camera move towards the target, smoothly. Expected to be called once per frame.
	pub fn animate(&mut self, frame_dt: Duration) {
		// What portion of the remaining vector should we travel?
		let update_factor = (self.speed * frame_dt.as_secs_f32()).min(1.0);
		let next_position =
			self.current_position * (1.0 - update_factor) + self.target_position * update_factor;
		// Make sure we move enough so that we avoid an annoying visual effect.
		// If we let the camera get slower and slower as it gets closer to the target,
		// it eventually goes slow enough so that it only moves at a very few pixels every second,
		// making each pixel jump noticable, which looks bad.
		let min_pixels_traveled = 0.2;
		let mut delta = next_position - self.current_position;
		let mut delta_length = delta.length();
		if delta_length == 0.0 {
			// Avoid normalizing a zero-length vector,
			// NaN poisioning is a curse no one should endure.
			return;
		}
		delta_length = delta_length.max(min_pixels_traveled / self.tile_size_px());
		let dist = self.current_position.distance(self.target_position);
		if dist < delta_length * 1.6 {
			// Make sure we eventually get exactly to the target.
			self.current_position = self.target_position;
			return;
		}
		delta = delta.normalize() * delta_length;
		self.current_position += delta;
	}

	/// Sets the target on some new world state via some info about that state.
	pub fn set_target(&mut self, info: &InfoForCamera) {
		if let Some(player_position) = info.player_position {
			self.target_position = player_position;
		}
	}

	/// Sets the target on some initial world state via some info about that state.
	pub fn set_initial_target(&mut self, info: &InfoForCamera) {
		if let Some(player_position) = info.player_position {
			self.target_position = player_position;
			self.current_position = player_position;
		}
	}
}
