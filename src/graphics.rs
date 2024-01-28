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

/// An instance of a sprite that has a position, depth layer and animations.
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

/// The world, as a set of animated sprites, to be displayed.
/// It represents a logical world or even a transition to a logical world,
/// but the logical nature of things is lost to sprites, it is a render in a sense.
pub struct GraphicalWorld {
	sprites: Vec<DisplayedSprite>,
}

impl GraphicalWorld {
	pub fn new() -> GraphicalWorld {
		GraphicalWorld { sprites: vec![] }
	}

	pub fn from_logical_world(lw: &LogicalWorld) -> GraphicalWorld {
		let transition = LogicalTransition { resulting_lw: lw.clone(), logical_events: vec![] };
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
		// We iterate over all the tiles, creating sprites to represent their content.
		for (coords, tile) in transition.resulting_lw.tiles() {
			// Ground.
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
			// Object.
			if let Some(obj) = tile.obj.as_ref() {
				let sprite_from_sheet = match obj {
					Obj::Wall => SpriteFromSheet::Wall,
					Obj::Sword => SpriteFromSheet::Sword,
					Obj::Shield => SpriteFromSheet::Shield,
					Obj::Rock => SpriteFromSheet::Rock,
					Obj::Bunny { .. } => SpriteFromSheet::Bunny,
					Obj::Slime { .. } => SpriteFromSheet::Slime,
				};
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
					move_animation,
					fail_to_move_animation,
					hit_animation,
					None,
				));
			}
		}
		for logical_event in transition.logical_events.iter() {
			match logical_event {
				// When damages are dealt, a damage number shall appear and float away.
				LogicalEvent::Killed { at, damages, .. } | LogicalEvent::Hit { at, damages, .. } => {
					gw.add_sprite(DisplayedSprite::new(
						SpriteFromSheet::Digit(*damages as u8),
						at.as_vec2(),
						DepthLayer::TemporaryText,
						None,
						None,
						None,
						Some(TemporaryTextAnimation::new(
							at.as_vec2() + Vec2::new(0.0, -0.5),
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

	/// Render the rendering!
	pub fn draw(
		&self,
		_ctx: &mut Context,
		canvas: &mut Canvas,
		spritesheet_stuff: &SpritesheetStuff,
	) -> GameResult {
		let sprite_px_scaled_to_how_many_screen_px = 7.0;
		let sprite_size_px = 8.0 * sprite_px_scaled_to_how_many_screen_px;
		for sprite in self.sprites.iter() {
			if !sprite.visible() {
				continue;
			}
			let center = sprite.center();
			let top_left = center * sprite_size_px - Vec2::new(1.0, 1.0) * sprite_size_px / 2.0;
			let top_left = top_left + Vec2::new(400.0, 400.0);
			let plain_color = sprite.plain_color();
			let (spritesheet, color) = if let Some(color) = plain_color {
				// A plain color shall be multiplied to the sprite, but we want all the sprite
				// to be exactly of that *plain* color, so we choose a variant of the sprite that
				// is all white. We find it in the spritesheet that was painted in white.
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
			time_interval: TimeInterval::with_duration(Duration::from_secs_f32(0.3)),
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
