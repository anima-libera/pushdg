//! The logical aspects of the game,
//! the mechanics and gameplay without graphical considerations.
//!
//! The idea is that the game state is not really mutated when something happens,
//! it is rather used to produce state transitions that contain logical descriptions
//! of what happen. These are used to animate the rendering of the state.

use std::collections::{hash_map::Entry, HashMap};

use ggez::glam::IVec2;
use rand::seq::SliceRandom;

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

	/// Can the player see over it?
	fn blocks_vision(&self) -> bool {
		matches!(self, Obj::Wall)
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

/// Every tile has a ground, below the potential object. The ground does not move.
#[derive(Clone)]
pub enum Ground {
	/// The classic ground, nothing special.
	Floor,
	// TODO: Hole, Ice, FragileFloor
}

#[derive(Clone)]
pub struct Tile {
	pub ground: Ground,
	pub obj: Option<Obj>,
	pub visible: bool,
}

impl Tile {
	pub fn floor() -> Tile {
		Tile { ground: Ground::Floor, obj: None, visible: false }
	}
	pub fn obj(obj: Obj) -> Tile {
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

	pub fn place_tile(&mut self, coords: IVec2, tile: Tile) {
		self.grid.insert(coords, tile);
	}
	pub fn place_tile_no_overwrite(&mut self, coords: IVec2, tile: Tile) {
		if let Entry::Vacant(vacant) = self.grid.entry(coords) {
			vacant.insert(tile);
		}
	}

	pub fn tiles(&self) -> impl Iterator<Item = (IVec2, &Tile)> {
		self.grid.iter().map(|(&coords, tile)| (coords, tile))
	}

	fn player_coords(&self) -> Option<IVec2> {
		self.grid.iter().find_map(|(&coords, tile)| {
			tile.obj.as_ref().is_some_and(|obj| matches!(obj, Obj::Bunny { .. })).then_some(coords)
		})
	}

	/// Computes the visibility of the tiles.
	fn updated_visibility(mut self) -> LogicalWorld {
		let player_coords = self.player_coords();
		// First pass, most of the vision is established here.
		let lw_clone = self.clone();
		for (coords, tile) in self.grid.iter_mut() {
			tile.visible = if let Some(player_coords) = player_coords {
				let dist = player_coords.as_vec2().distance(coords.as_vec2());
				if dist == 0.0 {
					true
				} else {
					// Only tiles in this radius may become visible.
					dist <= 6.5 && {
						let direction = (coords.as_vec2() - player_coords.as_vec2()).normalize();
						let step = 0.1;
						let mut point = player_coords.as_vec2();
						loop {
							if point.distance(coords.as_vec2()) < 3.0 * step {
								// A line of sight was established, we got vision here.
								break true;
							}
							let point_coords = point.round().as_ivec2();
							if lw_clone.grid.get(&point_coords).is_some_and(|tile| {
								tile.obj.as_ref().is_some_and(|obj| obj.blocks_vision())
							}) {
								// A vision-blocking object is blocking the line of sight.
								break point_coords == *coords;
							}
							point += direction * step;
						}
					}
				}
			} else {
				true
			};
		}
		// Second pass, add vision to some vision-blocking objects,
		// mostly for aesthetic purposes.
		let lw_clone = self.clone();
		for (coords, tile) in self.grid.iter_mut() {
			if let Some(player_coords) = player_coords {
				let dist = player_coords.as_vec2().distance(coords.as_vec2());
				if dist <= 6.5
					&& lw_clone.grid.get(coords).is_some_and(|tile| {
						!tile.visible && tile.obj.as_ref().is_some_and(|obj| obj.blocks_vision())
					}) {
					for to_adjecent in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
						let to_adjecent = IVec2::from(to_adjecent);
						let adjacent_coords = *coords + to_adjecent;
						if lw_clone.grid.get(&adjacent_coords).is_some_and(|tile| {
							tile.visible
								&& (tile.obj.as_ref().is_some_and(|obj| !obj.blocks_vision())
									|| tile.obj.is_none())
						}) {
							tile.visible = true;
							break;
						}
					}
				}
			}
		}
		// Third pass, add vision to some vision-blocking objects in corners of visible
		// vision-blocking objects, entierly for aesthetic purposes.
		let lw_clone = self.clone();
		for (coords, tile) in self.grid.iter_mut() {
			if let Some(player_coords) = player_coords {
				let dist = player_coords.as_vec2().distance(coords.as_vec2());
				if dist <= 6.5
					&& lw_clone.grid.get(coords).is_some_and(|tile| {
						!tile.visible && tile.obj.as_ref().is_some_and(|obj| obj.blocks_vision())
					}) {
					for to_adjecent in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
						let to_adjecent = IVec2::from(to_adjecent);
						let adjacent_coords = *coords + to_adjecent;
						if lw_clone.grid.get(&adjacent_coords).is_some_and(|tile| {
							tile.visible && tile.obj.as_ref().is_some_and(|obj| obj.blocks_vision())
						}) && lw_clone.grid.get(&(*coords + to_adjecent.perp())).is_some_and(|tile| {
							tile.visible && tile.obj.as_ref().is_some_and(|obj| obj.blocks_vision())
						}) {
							// Corner that would look better if visible detected, granting visibility.
							tile.visible = true;
							break;
						}
					}
				}
			}
		}
		self
	}

	/// Returns the transition of the player trying to move in the given direction.
	pub fn player_move(&self, direction: IVec2) -> LogicalTransition {
		if let Some(coords) = self.player_coords() {
			let player_force = 2;
			self.try_to_move(coords, direction, player_force).updated_visibility()
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
						res_lw.try_to_move(*coords, direction, argent_force).updated_visibility()
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
		if self.grid.get(&dst).is_some_and(|tile| tile.obj.as_ref().is_some_and(|obj| obj.is_enemy()))
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
	fn try_to_move(&self, mover_coords: IVec2, direction: IVec2, force: i32) -> LogicalTransition {
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
				logical_events.push(LogicalEvent::FailToMove { from: coords, to: coords + direction });
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

impl LogicalTransition {
	pub fn updated_visibility(self) -> LogicalTransition {
		LogicalTransition {
			resulting_lw: self.resulting_lw.updated_visibility(),
			logical_events: self.logical_events,
		}
	}
}
