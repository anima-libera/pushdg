use gameplay::{LogicalTransition, LogicalWorld};
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
	use rand::seq::SliceRandom;

	/// Every tile has a ground, below the potential object. The ground does not move.
	#[derive(Clone)]
	pub enum Ground {
		/// The classic ground, nothing special.
		Floor,
		// TODO: Hole, Ice, FragileFloor
	}

	/// A tile can have zero or one object on it, and these can be moved.
	#[derive(Clone)]
	pub enum Obj {
		/// Hard to move, it just stays there, being a wall.
		Wall,
		/// Does more damages. Great weapon, terrible for protection.
		Sword,
		/// Does zero damages. Great for protection, terrible weapon.
		Shield,
		/// The average pushable object, has the default stat for every stat.
		Rock,
		/// The player. We play as a bunny. It is cute! :3
		Bunny { hp: i32 },
		/// The basic enemy.
		Slime {
			hp: i32,
			/// This token indicates that this agent has yet to make a move.
			move_token: bool,
		},
	}

	impl Obj {
		/// When a pusher wants to push one or more objects, the sum of the masses of the
		/// objects that may be pushed is compared to the force of the pusher to see if the
		/// pusher succeeds to push (force >= total mass) or fails to push (force < total mass).
		fn mass(&self) -> i32 {
			match self {
				Obj::Wall => 10,
				Obj::Slime { .. } => 3,
				Obj::Bunny { .. } => 3,
				_ => 1,
			}
		}

		/// When an object W is failed to be pushed into an object T, W may deal damages to T
		/// if T is the kind of object that may take damages.
		fn damages(&self) -> i32 {
			match self {
				Obj::Sword => 3,
				Obj::Shield => 0,
				Obj::Slime { .. } => 2,
				_ => 1,
			}
		}

		/// An object may take damages if it has some HP.
		fn hp(&self) -> Option<i32> {
			match self {
				Obj::Bunny { hp } => Some(*hp),
				Obj::Slime { hp, .. } => Some(*hp),
				_ => None,
			}
		}

		/// Doesn't check if HP goes down to zero or lower,
		/// killing hits should be handled by hand.
		fn take_damage(&mut self, damages: i32) {
			match self {
				Obj::Bunny { hp } => *hp -= damages,
				Obj::Slime { hp, .. } => *hp -= damages,
				_ => {},
			}
		}

		/// Some agents may be neutral, this only flags agents that are hostile to the player.
		fn is_enemy(&self) -> bool {
			matches!(self, Obj::Slime { .. })
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

	/// A logical state of the world, with no regards to rendering or animation.
	/// The world is a grid of tiles.
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

		/// Returns the transition of the player trying to move in the given direction.
		pub fn player_move(&self, direction: IVec2) -> LogicalTransition {
			if let Some(coords) = self.player_coords() {
				let player_force = 2;
				self.try_to_move(coords, direction, player_force)
			} else {
				LogicalTransition { resulting_lw: self.clone(), logical_events: vec![] }
			}
		}

		/// When it is the game's turn to play, agents are given one move token
		/// so that one agent doesn't get to move twice.
		pub fn give_move_token_to_agents(&mut self) {
			for tile in self.grid.values_mut() {
				if let Some(obj) = tile.obj.as_mut() {
					obj.give_move_token();
				}
			}
		}

		/// If there are still agents that can move,
		/// then returns the transition of one trying to move, chosen randomly.
		pub fn handle_move_for_one_agent(&mut self) -> Option<LogicalTransition> {
			let mut keys: Vec<_> = self.grid.keys().collect();
			keys.shuffle(&mut rand::thread_rng());
			for coords in keys.into_iter() {
				let tile = self.grid.get(coords).unwrap();
				if let Some(obj) = tile.obj.as_ref() {
					if obj.has_move_token() {
						let mut res_lw = self.clone();
						res_lw.grid.get_mut(coords).unwrap().obj.as_mut().unwrap().take_move_token();
						return Some(if let Some(direction) = self.ai_decision(*coords) {
							let argent_force = 2;
							res_lw.try_to_move(*coords, direction, argent_force)
						} else {
							LogicalTransition { resulting_lw: res_lw, logical_events: vec![] }
						});
					}
				}
			}
			None
		}

		/// Test simple AI.
		fn ai_decision(&self, agent_coords: IVec2) -> Option<IVec2> {
			let target_coords = self.player_coords()?;
			// Move towards the target if it is in a streaight line.
			let direction = if agent_coords.x == target_coords.x {
				if target_coords.y < agent_coords.y {
					IVec2::new(0, -1)
				} else {
					IVec2::new(0, 1)
				}
			} else if agent_coords.y == target_coords.y {
				if target_coords.x < agent_coords.x {
					IVec2::new(-1, 0)
				} else {
					IVec2::new(1, 0)
				}
			} else {
				return None;
			};
			// Avoid bumping into an other enemy, it may help the player.
			let dst = agent_coords + direction;
			if self
				.grid
				.get(&dst)
				.is_some_and(|tile| tile.obj.as_ref().is_some_and(|obj| obj.is_enemy()))
			{
				return None;
			}
			Some(direction)
		}

		/// In the case of an object W at `weapon_coords` hitting an object T
		/// as it tries to move in the given `direction`, a hit may occur, and it may be a killing
		/// hit, depending on hittability of T, the damages of W, the remaining HP of T.
		/// This returns what would happen, would the hit occur.
		fn what_would_happen_if_hit(
			&self,
			weapon_coords: IVec2,
			direction: IVec2,
		) -> HitAttemptConsequences {
			let weapon_obj = self.grid.get(&weapon_coords).as_ref().unwrap().obj.as_ref().unwrap();
			let target_coords = weapon_coords + direction;
			if let Some(target_obj) = self.grid.get(&target_coords).as_ref().unwrap().obj.as_ref() {
				if let Some(target_hp) = target_obj.hp() {
					let damages = weapon_obj.damages();
					if target_hp <= damages {
						// HP would drop to zero or less.
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

		/// When an object tries to move in some direction, depending on a lot of factors
		/// like the force of the object, what may block its path, then a push or even a hit
		/// could succeed, fail, implicate some amount of objects, etc.
		/// This returns what would happen.
		fn what_would_happen_if_try_to_move(
			&self,
			mover_coords: IVec2,
			direction: IVec2,
			force: i32,
		) -> MoveAttemptConsequences {
			let mut coords = mover_coords;
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
							// The final object of the chain that would have been pushed if the push
							// if not for the target would now hit the target.
							final_hit = self.what_would_happen_if_hit(coords - direction, direction);
							break match final_hit {
								HitAttemptConsequences::Kill { .. } => {
									// The target is killed, and as a design choice I find it cool
									// that since now what was blocking is no more then the push
									// happens now.
									true
								},
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

		/// Returns the transition of the object at the given coords trying to move
		/// in the given direction and with the given force.
		fn try_to_move(
			&self,
			mover_coords: IVec2,
			direction: IVec2,
			force: i32,
		) -> LogicalTransition {
			let mut res_lw = self.clone();
			let mut logical_events = vec![];
			let MoveAttemptConsequences { success, length, final_hit } =
				self.what_would_happen_if_try_to_move(mover_coords, direction, force);
			let mut coords = mover_coords;
			let mut previous_obj = None;
			for _ in 0..length {
				if success {
					// The push is successful so each object in the chain is replaced
					// by the previous object, and gets to replace the next object.
					std::mem::swap(
						&mut previous_obj,
						&mut res_lw.grid.get_mut(&coords).unwrap().obj,
					);
					if previous_obj.is_some() {
						logical_events.push(LogicalEvent::Move { from: coords, to: coords + direction });
					}
				} else {
					// The push is not successful, but the objects that fail to move still
					// has to fail to move (important because it ultimately results in an
					// animation that displays the objects failing to move, and also
					// which objects fail to move and which are not even concerned).
					logical_events
						.push(LogicalEvent::FailToMove { from: coords, to: coords + direction });
				}
				coords += direction;
			}
			// We are at the end of the push chain. There may be a hit happening there,
			// with the last object moving or failing to move hitting what comes after.
			if success {
				std::mem::swap(
					&mut previous_obj,
					&mut res_lw.grid.get_mut(&coords).unwrap().obj,
				);
				match final_hit {
					HitAttemptConsequences::Kill { damages } => {
						// The hit kills the blocking object, allowing the push to succeed
						// and the last object of the push chain to take the place of the target.
						let target_obj = previous_obj.take().unwrap();
						logical_events.push(LogicalEvent::Killed {
							obj: target_obj,
							at: coords,
							hit_direction: direction,
							damages,
						});
					},
					HitAttemptConsequences::NonLethalHit { .. }
					| HitAttemptConsequences::TargetIsNotHittable => {
						unreachable!(
							"If there is a non-killed target, then the push would have been a failure"
						)
					},
					HitAttemptConsequences::ThereIsNoTraget => assert!(previous_obj.is_none()),
				}
				assert!(previous_obj.is_none());
			} else {
				match final_hit {
					HitAttemptConsequences::NonLethalHit { damages } => {
						let target_obj = res_lw.grid.get_mut(&coords).unwrap().obj.as_mut().unwrap();
						target_obj.take_damage(damages);
						logical_events.push(LogicalEvent::Hit {
							at: coords,
							hit_direction: direction,
							damages,
						});
					},
					HitAttemptConsequences::TargetIsNotHittable => {},
					HitAttemptConsequences::Kill { .. } | HitAttemptConsequences::ThereIsNoTraget => {
						unreachable!(
							"If there is no or no more target, \
							then nothing is blocking the push from succeeding"
						)
					},
				}
			}
			LogicalTransition { resulting_lw: res_lw, logical_events }
		}
	}

	enum HitAttemptConsequences {
		ThereIsNoTraget,
		/// Some targets cannot be hit in the sense that they do not have any HP
		/// and the notion of taking damages does not make sense for them.
		TargetIsNotHittable,
		NonLethalHit {
			damages: i32,
		},
		Kill {
			/// The target is killed, but this is still the damages dealt by the weapon,
			/// even if higher than the remaining HP of the killed target.
			damages: i32,
		},
	}

	struct MoveAttemptConsequences {
		/// Will some objects actually move or will they just fail to move?
		success: bool,
		/// The number of object that move or fail to move.
		length: i32,
		/// The frontmost object to move may hit an other object in front of it,
		/// if a hit happens and its consequences are also consequences of the move.
		final_hit: HitAttemptConsequences,
	}

	/// When something happens to turn a logical state of the world into an other,
	/// then a logical description of what happened (or even what failed to happen)
	/// can be useful to animate the transition.
	#[derive(Clone)]
	pub enum LogicalEvent {
		Move {
			from: IVec2,
			to: IVec2,
		},
		FailToMove {
			from: IVec2,
			to: IVec2,
		},
		Hit {
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

	/// When the player or agents move or something happens in the game,
	/// it results in a logical transition from a state to an other being produced
	/// instead of simply mutating the current state.
	/// This allows for animation to have access to all the events to animate,
	/// for the game to play all its moves and then the animations to play each of them
	/// taking some time, for the ai to play in its head and consider world states, etc.
	#[derive(Clone)]
	pub struct LogicalTransition {
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
		gameplay::{Ground, LogicalEvent, LogicalTransition, LogicalWorld, Obj},
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

	/// These refer to a sprite in the spritesheet.
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
			// Wild non-aligned sprites.
			if let SpriteFromSheet::Digit(digit) = self {
				let x = digit * 4;
				let y = 16;
				return Rect::new(x as f32 / 128.0, y as f32 / 128.0, 3.0 / 128.0, 5.0 / 128.0);
			}

			// Now we handle 8x8 sprites aligned on the 8x8-tiles grid.
			let (x, y) = match self {
				SpriteFromSheet::Wall => (0, 0),
				SpriteFromSheet::Floor => (0, 1),
				SpriteFromSheet::Sword => (1, 0),
				SpriteFromSheet::Shield => (2, 0),
				SpriteFromSheet::Rock => (3, 0),
				SpriteFromSheet::Bunny => (4, 0),
				SpriteFromSheet::Slime => (5, 0),
				SpriteFromSheet::Digit(_) => unreachable!("Handled above"),
			};
			Rect::new(
				x as f32 * 8.0 / 128.0,
				y as f32 * 8.0 / 128.0,
				8.0 / 128.0,
				8.0 / 128.0,
			)
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

		// Paint the spritesheet in white.
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
	spritesheet_stuff: SpritesheetStuff,
}

impl Game {
	fn new(ctx: &mut Context) -> GameResult<Game> {
		let lw = LogicalWorld::new_test();
		let gw = GraphicalWorld::from_logical_world(&lw);
		let spritesheet_stuff = SpritesheetStuff::new(ctx)?;
		let phase = Phase::WaitingForPlayerToMakeAMove;
		Ok(Game { logical_world: lw, phase, graphical_world: gw, spritesheet_stuff })
	}

	fn player_move(&mut self, direction: IVec2) {
		if matches!(self.phase, Phase::WaitingForPlayerToMakeAMove) {
			let mut transition = self.logical_world.player_move(direction);
			self.graphical_world = GraphicalWorld::from_logical_world_transition(&transition);
			self.logical_world = transition.resulting_lw.clone();

			// Play all the moves of everyting that is not a player up until the player's next turn.
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
	fn update(&mut self, _ctx: &mut Context) -> GameResult {
		let no_more_animations = !self.graphical_world.has_animation();
		if no_more_animations {
			if let Phase::WaitingForAnimationsToFinish(next_tranitions) = &mut self.phase {
				if next_tranitions.is_empty() {
					self.phase = Phase::WaitingForPlayerToMakeAMove;
				} else {
					let transition = next_tranitions.remove(0);
					self.graphical_world = GraphicalWorld::from_logical_world_transition(&transition);
					self.logical_world = transition.resulting_lw.clone();
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
		self.graphical_world.draw(ctx, &mut canvas, &self.spritesheet_stuff)?;
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
