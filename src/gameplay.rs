//! The logical aspects of the game,
//! the mechanics and gameplay without graphical considerations.
//!
//! The idea is that the game state is not really mutated when something happens,
//! it is rather used to produce state transitions that contain logical descriptions
//! of what happen. These are used to animate the rendering of the state.

use std::collections::{hash_map::Entry, HashMap};

use ggez::glam::IVec2;
use rand::seq::SliceRandom;

use crate::generation::filled_rect;

/// A tile can have zero or one object on it, and these can be moved.
#[derive(Clone)]
pub enum Obj {
	/// Hard to move, it just stays there, being a wall.
	Wall,
	/// Does more damages. Great weapon, terrible for protection.
	Sword,
	/// Does zero damages. Great for protection, terrible weapon.
	Shield,
	/// Can mine walls.
	Pickaxe,
	/// The average pushable object, has the default stat for every stat.
	Rock,
	/// An exit door that objects can go through to go to the next level.
	Exit,
	/// Gem that grants wall-through vision to the player if adjacent.
	VisionGem,
	/// Restores health when consumed.
	Heart,
	/// Grants a redo.
	RedoHeart,
	/// Like a wall but can be opened by a key.
	Door,
	/// Can open a door.
	Key,
	/// Pulls and is pulled.
	Rope,
	/// Vision-blocking pushable object.
	Bush,
	/// The player. We play as a bunny. It is cute! :3
	Bunny { hp: i32, max_hp: i32 },
	/// The basic enemy.
	Slime {
		hp: i32,
		/// This token indicates that this agent has yet to make a move.
		move_token: bool,
	},
	/// An other enemy, mushroom themed.
	Shroomer {
		hp: i32,
		/// This token indicates that this agent has yet to make a move.
		move_token: bool,
	},
	/// Mushroom. A production of the shroomer.
	Shroom {
		/// This token indicates that this agent has yet to make a move.
		move_token: bool,
	},
	/// Fish that moves on its own.
	Fish {
		direction: IVec2,
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
			Obj::Wall | Obj::Door | Obj::Shroom { .. } => 10,
			Obj::Bunny { .. } | Obj::Slime { .. } | Obj::Shroomer { .. } => 3,
			_ => 1,
		}
	}

	/// When an object W is failed to be pushed into an object T, W may deal damages to T
	/// if T is the kind of object that may take damages.
	fn damages(&self) -> i32 {
		match self {
			Obj::Sword => 3,
			Obj::Shield | Obj::Exit | Obj::Heart | Obj::RedoHeart => 0,
			Obj::Slime { .. } => 2,
			Obj::Shroomer { .. } => 2,
			_ => 1,
		}
	}

	/// An object may take damages if it has some HP.
	fn hp(&self) -> Option<i32> {
		match self {
			Obj::Bunny { hp, .. } | Obj::Slime { hp, .. } | Obj::Shroomer { hp, .. } => Some(*hp),
			_ => None,
		}
	}

	/// Doesn't check if HP goes down to zero or lower,
	/// killing hits should be handled by hand.
	fn take_damage(&mut self, damages: i32) {
		match self {
			Obj::Bunny { hp, .. } | Obj::Slime { hp, .. } | Obj::Shroomer { hp, .. } => *hp -= damages,
			_ => {},
		}
	}

	/// Can the player see over it?
	fn blocks_vision(&self) -> bool {
		matches!(self, Obj::Wall | Obj::Bush)
	}

	/// Some agents may be neutral, this only flags agents that are hostile to the player.
	fn is_enemy(&self) -> bool {
		matches!(self, Obj::Slime { .. } | Obj::Shroomer { .. })
	}

	fn give_move_token(&mut self) {
		match self {
			Obj::Slime { move_token, .. }
			| Obj::Shroomer { move_token, .. }
			| Obj::Shroom { move_token }
			| Obj::Fish { move_token, .. } => *move_token = true,
			_ => {},
		}
	}

	fn has_move_token(&self) -> bool {
		match self {
			Obj::Slime { move_token, .. }
			| Obj::Shroomer { move_token, .. }
			| Obj::Shroom { move_token }
			| Obj::Fish { move_token, .. } => *move_token,
			_ => false,
		}
	}

	fn take_move_token(&mut self) -> bool {
		match self {
			Obj::Slime { move_token, .. }
			| Obj::Shroomer { move_token, .. }
			| Obj::Shroom { move_token }
			| Obj::Fish { move_token, .. } => {
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
	pub redo_count: i32,
	pub max_redo_count: i32,
}

impl LogicalWorld {
	pub fn new_empty() -> LogicalWorld {
		LogicalWorld { grid: HashMap::new(), redo_count: 3, max_redo_count: 9 }
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
	pub fn tile(&self, coords: IVec2) -> Option<&Tile> {
		self.grid.get(&coords)
	}
	pub fn obj(&self, coords: IVec2) -> Option<&Obj> {
		self.grid.get(&coords).and_then(|tile| tile.obj.as_ref())
	}

	fn player_coords(&self) -> Option<IVec2> {
		self.grid.iter().find_map(|(&coords, tile)| {
			tile.obj.as_ref().is_some_and(|obj| matches!(obj, Obj::Bunny { .. })).then_some(coords)
		})
	}

	pub fn has_player(&self) -> bool {
		self.player_coords().is_some()
	}

	/// Computes the visibility of the tiles.
	fn updated_visibility(mut self) -> LogicalWorld {
		// TODO: Make this whole function more readable.
		let player_coords = self.player_coords();

		// Handle vision gem effect.
		// If the player is adjacent to a vision gem then they get see-through vision.
		if let Some(player_coords) = player_coords {
			let adjacent_to_vision_gem = 'vision_gem: {
				for to_adjecent in four_directions() {
					let adjacent_coords = player_coords + to_adjecent;
					if let Some(Obj::VisionGem) = self.obj(adjacent_coords) {
						break 'vision_gem true;
					}
				}
				false
			};
			if adjacent_to_vision_gem {
				for (coords, tile) in self.grid.iter_mut() {
					let dist = player_coords.as_vec2().distance(coords.as_vec2());
					tile.visible = dist <= 6.5;
				}
				return self;
			}
		}

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
							if lw_clone.obj(point_coords).is_some_and(|obj| obj.blocks_vision()) {
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
					for to_adjecent in four_directions() {
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
					for to_adjecent in four_directions() {
						// Sorry for the very bad code here,
						// it could do with lots of cleanup,
						// for the story, it makes sure that the corner that we are about
						// to make visible despite it being out of sight is a corner that
						// would complete the corner of a piece of room in which the player is.
						// TODO: Make this more readable.
						let adjacent_coords = *coords + to_adjecent;
						let other_adjacent_coords = *coords + to_adjecent.perp();
						let corner_coords = *coords + to_adjecent + to_adjecent.perp();
						let coords_dist = coords.as_vec2().distance(player_coords.as_vec2());
						let adjacent_dist = adjacent_coords.as_vec2().distance(player_coords.as_vec2());
						let other_adjacent_dist =
							other_adjacent_coords.as_vec2().distance(player_coords.as_vec2());
						let corner_dist = corner_coords.as_vec2().distance(player_coords.as_vec2());
						let min_dist_is_corner =
							corner_dist < coords_dist.min(adjacent_dist).min(other_adjacent_dist);
						if lw_clone.grid.get(&adjacent_coords).is_some_and(|tile| {
							tile.visible && tile.obj.as_ref().is_some_and(|obj| obj.blocks_vision())
						}) && lw_clone.grid.get(&other_adjacent_coords).is_some_and(|tile| {
							tile.visible && tile.obj.as_ref().is_some_and(|obj| obj.blocks_vision())
						}) && min_dist_is_corner
							&& lw_clone.grid.get(&corner_coords).is_some_and(|tile| {
								tile.visible
									&& (tile.obj.is_none()
										|| tile.obj.as_ref().is_some_and(|obj| !obj.blocks_vision()))
							}) {
							// Corner that would look better if visible, granting visibility.
							tile.visible = true;
							break;
						}
					}
				}
			}
		}
		self
	}

	/// There are walls everywhere, we apply that design choice here.
	fn generated_walls_outside(mut self) -> LogicalWorld {
		let keys: Vec<_> = self.grid.keys().copied().collect();
		for coords in keys {
			if !matches!(self.obj(coords), Some(Obj::Wall)) {
				for coords in filled_rect(coords - IVec2::new(1, 1), IVec2::new(3, 3)) {
					self.place_tile_no_overwrite(coords, Tile::obj(Obj::Wall));
				}
			}
		}
		self
	}

	/// Returns the transition of the player trying to move in the given direction.
	pub fn player_move(&self, direction: IVec2) -> LogicalTransition {
		if let Some(coords) = self.player_coords() {
			let player_force = 2;
			self
				.try_to_move(coords, direction, player_force)
				.generated_walls_outside()
				.updated_visibility()
		} else {
			self.clone().into()
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
					let is_shroom = matches!(res_lw.obj(*coords), Some(Obj::Shroom { .. }));
					let is_shroomer = matches!(res_lw.obj(*coords), Some(Obj::Shroomer { .. }));
					let is_fish = matches!(res_lw.obj(*coords), Some(Obj::Fish { .. }));
					let direction = if is_shroom {
						self.shroom_ai_decision(*coords)
					} else if is_fish {
						self.fish_ai_decision(*coords)
					} else {
						self.ai_decision(*coords)
					};
					return Some(if let Some(direction) = direction {
						let target_coords = *coords + direction;
						let target_is_bunny =
							matches!(res_lw.obj(target_coords), Some(Obj::Bunny { .. }));
						if is_shroom || (is_shroomer && target_is_bunny) {
							res_lw.sacrifice_hit(*coords, direction).updated_visibility()
						} else {
							let argent_force = 2;
							res_lw.try_to_move(*coords, direction, argent_force).updated_visibility()
						}
					} else {
						res_lw.into()
					});
				}
			}
		}
		None
	}

	/// Simple enemy AI.
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
		// No vision through vision-blocking objects.
		let vision_blocked = {
			let mut coords = agent_coords;
			loop {
				coords += direction;
				if coords == target_coords {
					break false;
				} else if self.obj(coords).is_some_and(|obj| obj.blocks_vision()) {
					break true;
				}
			}
		};
		if vision_blocked {
			return None;
		}
		// All good, can move forward!
		Some(direction)
	}

	/// Shroom AI.
	fn shroom_ai_decision(&self, agent_coords: IVec2) -> Option<IVec2> {
		let target_coords = self.player_coords()?;
		let direction = target_coords - agent_coords;
		// Attack the player if adjacent.
		(direction.x.abs() + direction.y.abs() == 1).then_some(direction)
	}

	/// Fish AI.
	fn fish_ai_decision(&self, agent_coords: IVec2) -> Option<IVec2> {
		let direction = if let Some(Obj::Fish { direction, .. }) = self.obj(agent_coords) {
			*direction
		} else {
			return None;
		};
		let dst_coords = agent_coords + direction;
		let target_coords = self.player_coords()?;
		let noting_ahead = self.grid.get(&dst_coords).is_some() && self.obj(dst_coords).is_none();
		if target_coords == dst_coords || noting_ahead {
			Some(direction)
		} else {
			Some(-direction)
		}
	}

	/// If the source object was pushed into the destination object in a blocked push, then what?
	fn what_would_happen_if_interact(
		&self,
		src_obj: &Obj,
		dst_obj: &Obj,
		dst_coords: IVec2,
	) -> Option<InteractionConsequences> {
		if matches!(dst_obj, Obj::Exit) {
			Some(InteractionConsequences::Exit { at: dst_coords })
		} else if matches!((src_obj, dst_obj), (Obj::Pickaxe, Obj::Wall)) {
			Some(InteractionConsequences::Mine)
		} else if matches!((src_obj, dst_obj), (Obj::Key, Obj::Door)) {
			Some(InteractionConsequences::KeyOpenDoor)
		} else if matches!((src_obj, dst_obj), (Obj::Bunny { .. }, Obj::Heart)) {
			Some(InteractionConsequences::Heal)
		} else if matches!((src_obj, dst_obj), (Obj::Bunny { .. }, Obj::RedoHeart)) {
			Some(InteractionConsequences::GainARedo)
		} else if matches!(dst_obj, Obj::Shroom { .. }) {
			Some(InteractionConsequences::StompShroom)
		} else if let Some(target_hp) = dst_obj.hp() {
			let damages = src_obj.damages();
			if target_hp <= damages {
				// HP would drop to zero or less.
				Some(InteractionConsequences::Kill { damages })
			} else {
				Some(InteractionConsequences::NonLethalHit { damages })
			}
		} else {
			None
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
		// Push.
		let mut coords = mover_coords;
		let mut remaining_force = force;
		let mut length = 0;
		let mut length_removed_due_to_interaction = 0;
		let mut final_interaction = None;
		let success = 'success: loop {
			coords += direction;
			length += 1;
			if let Some(dst_tile) = self.grid.get(&coords) {
				if let Some(dst_obj) = dst_tile.obj.as_ref() {
					remaining_force -= dst_obj.mass();
					if remaining_force < 0 {
						// All the force of the pusher was used up, nothing more can be pushed.
						// Now we scan the pushed chain backwards for an interaction.
						while length_removed_due_to_interaction < length {
							let src_coords = coords - direction;
							let src_obj = self.obj(src_coords).unwrap();
							let dst_obj = self.obj(coords).unwrap();
							// The final object of the chain that would have been pushed but is blocked by
							// the target now try to interact with the target.
							final_interaction =
								self.what_would_happen_if_interact(src_obj, dst_obj, coords);
							if let Some(final_interaction) = final_interaction.as_ref() {
								// Depending on the interaction, the move may succeed or not.
								break 'success final_interaction.allows_move();
							}
							length_removed_due_to_interaction += 1;
							coords -= direction;
						}
						break false;
					}
				} else {
					break true;
				}
			} else {
				break false;
			}
		};
		if final_interaction.is_some() {
			length -= length_removed_due_to_interaction;
		}
		let non_pulled_length = length;
		// Pull.
		let mut coords = mover_coords;
		let mut remaining_force = force;
		let mut pulled_length = 0;
		let mut can_pull_next = false;
		loop {
			coords -= direction;
			if let Some(dst_obj) = self.obj(coords) {
				if matches!(dst_obj, Obj::Rope) || can_pull_next {
					can_pull_next = false;
					remaining_force -= dst_obj.mass();
					if remaining_force < 0 {
						break;
					}
					pulled_length += 1;
					if matches!(dst_obj, Obj::Rope) {
						can_pull_next = true;
					}
				} else {
					break;
				}
			} else {
				break;
			}
		}
		MoveAttemptConsequences { success, non_pulled_length, pulled_length, final_interaction }
	}

	/// Returns the transition of the object at the given coords trying to move
	/// in the given direction and with the given force.
	fn try_to_move(&self, mover_coords: IVec2, direction: IVec2, force: i32) -> LogicalTransition {
		let mut res_lw = self.clone();
		let mut logical_events = vec![];
		let MoveAttemptConsequences { success, non_pulled_length, pulled_length, final_interaction } =
			self.what_would_happen_if_try_to_move(mover_coords, direction, force);
		let mut coords = mover_coords;
		let mut previous_obj = None;
		for _ in 0..non_pulled_length {
			if success {
				// The push is successful so each object in the chain is replaced
				// by the previous object, and gets to replace the next object.
				std::mem::swap(
					&mut previous_obj,
					&mut res_lw.grid.get_mut(&coords).unwrap().obj,
				);
				previous_obj = match previous_obj.take() {
					Some(Obj::Fish { move_token, .. }) => Some(Obj::Fish { direction, move_token }),
					x => x,
				};
				let is_exiting = if let Some(InteractionConsequences::Exit { at }) = final_interaction {
					at == coords + direction
				} else {
					false
				};
				if previous_obj.is_some() && !is_exiting {
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
		// We are at the end of the push chain. There may be an interaction happening there,
		// with the last object moving or failing to move interacting with what comes after.
		if success {
			std::mem::swap(
				&mut previous_obj,
				&mut res_lw.grid.get_mut(&coords).unwrap().obj,
			);
			if let Some(final_interaction) = final_interaction {
				match final_interaction {
					InteractionConsequences::Kill { damages } => {
						// The hit kills the blocking object, allowing the push to succeed
						// and the last object of the push chain to take the place of the target.
						let target_obj = previous_obj.take().unwrap();
						logical_events.push(LogicalEvent::Killed {
							obj: target_obj,
							at: coords,
							damages,
						});
					},
					InteractionConsequences::StompShroom => {
						let target_obj = previous_obj.take().unwrap();
						logical_events.push(LogicalEvent::Stomped { obj: target_obj, at: coords });
					},
					InteractionConsequences::Mine => {
						let target_obj = previous_obj.take().unwrap();
						logical_events.push(LogicalEvent::Mined { obj: target_obj, at: coords });
					},
					InteractionConsequences::KeyOpenDoor => {
						let key_obj = res_lw.grid.get_mut(&coords).unwrap().obj.take().unwrap();
						let door_obj = previous_obj.take().unwrap();
						logical_events.push(LogicalEvent::DoorOpenedWithKey {
							key_obj,
							door_obj,
							from: coords - direction,
							to: coords,
						});
					},
					InteractionConsequences::Exit { .. } => {
						std::mem::swap(
							&mut previous_obj,
							&mut res_lw.grid.get_mut(&coords).unwrap().obj,
						);
						let exiting_obj = previous_obj.take().unwrap();
						logical_events.push(LogicalEvent::Exit {
							obj: exiting_obj,
							from: coords - direction,
							to: coords,
						});
					},
					InteractionConsequences::Heal => {
						let _heart_obj = previous_obj.take().unwrap();
						let healed_obj = &mut res_lw.grid.get_mut(&coords).unwrap().obj.as_mut().unwrap();
						match healed_obj {
							Obj::Bunny { hp, max_hp } => *hp = *max_hp,
							_ => unreachable!("Only a bunny interacting with a heart can trigger a heal"),
						}
						logical_events.push(LogicalEvent::Healed { obj: healed_obj.clone(), at: coords });
					},
					InteractionConsequences::GainARedo => {
						let redo_heart_obj = previous_obj.take().unwrap();
						res_lw.redo_count = (self.redo_count + 1).clamp(0, self.max_redo_count);
						logical_events.push(LogicalEvent::RedoGained { obj: redo_heart_obj, at: coords });
					},
					InteractionConsequences::NonLethalHit { .. } => {
						unreachable!(
							"If there is a non-killed target, then the push would have been a failure"
						)
					},
				}
			}
			assert!(previous_obj.is_none());
		} else if let Some(final_interaction) = final_interaction {
			match final_interaction {
				InteractionConsequences::NonLethalHit { damages } => {
					let target_obj = res_lw.grid.get_mut(&coords).unwrap().obj.as_mut().unwrap();
					target_obj.take_damage(damages);
					logical_events.push(LogicalEvent::Hit { at: coords, damages });
				},
				InteractionConsequences::Kill { .. }
				| InteractionConsequences::Mine
				| InteractionConsequences::StompShroom
				| InteractionConsequences::KeyOpenDoor
				| InteractionConsequences::Heal
				| InteractionConsequences::GainARedo
				| InteractionConsequences::Exit { .. } => {
					unreachable!(
						"If there is no or no more target, \
  						then nothing is blocking the push from succeeding"
					)
				},
			}
		}
		// The pulling.
		if success {
			let mut coords = mover_coords;
			for _ in 0..pulled_length {
				coords -= direction;
				let obj = res_lw.grid.get_mut(&coords).unwrap().obj.take();
				res_lw.grid.get_mut(&(coords + direction)).unwrap().obj = obj;
				logical_events.push(LogicalEvent::Move { from: coords, to: coords + direction });
			}
		}
		// Shroomer tries to shroom.
		if matches!(self.obj(mover_coords), Some(Obj::Shroomer { .. }))
			&& res_lw.obj(mover_coords).is_none()
		{
			let adjacent_to_shroom = 'shroom: {
				for to_adjecent in four_directions() {
					let adjacent_coords = mover_coords + to_adjecent;
					if matches!(self.obj(adjacent_coords), Some(Obj::Shroom { .. })) {
						break 'shroom true;
					}
				}
				false
			};
			if !adjacent_to_shroom {
				res_lw.grid.get_mut(&mover_coords).unwrap().obj =
					Some(Obj::Shroom { move_token: false });
			}
		}
		// Done ^^.
		LogicalTransition { resulting_lw: res_lw, logical_events }
	}

	/// An object sacrifices itself to hit its target.
	fn sacrifice_hit(&self, hitter_coords: IVec2, direction: IVec2) -> LogicalTransition {
		let mut res_lw = self.clone();
		let mut logical_events = vec![];
		let hitter_obj = res_lw.grid.get_mut(&hitter_coords).unwrap().obj.take().unwrap();
		let target_coords = hitter_coords + direction;
		let damages = hitter_obj.damages();
		logical_events.push(LogicalEvent::MoveInto {
			obj: hitter_obj,
			from: hitter_coords,
			to: target_coords,
		});
		let target_obj = res_lw.grid.get_mut(&target_coords).unwrap().obj.as_mut().unwrap();
		target_obj.take_damage(damages);
		if target_obj.hp().unwrap() <= 0 {
			logical_events.push(LogicalEvent::Killed {
				obj: target_obj.clone(),
				at: target_coords,
				damages,
			});
			res_lw.grid.get_mut(&target_coords).unwrap().obj = None;
		} else {
			logical_events.push(LogicalEvent::Hit { at: target_coords, damages });
		}
		LogicalTransition { resulting_lw: res_lw, logical_events }
	}
}

enum InteractionConsequences {
	NonLethalHit {
		damages: i32,
	},
	Kill {
		/// The target is killed, but this is still the damages dealt by the weapon,
		/// even if higher than the remaining HP of the killed target.
		damages: i32,
	},
	/// Pickaxe mining a wall for example.
	Mine,
	/// A key is used to open a door, being consumed in the operation.
	KeyOpenDoor,
	/// Exit the level through an exit door.
	Exit {
		/// Coords of the exit door through which an object exits.
		at: IVec2,
	},
	/// Bunny ate a heart and is healed.
	Heal,
	/// Bunny ate a redo heart.
	GainARedo,
	/// Something stomps on a shroom, the poor thing.
	StompShroom,
}

impl InteractionConsequences {
	/// Does this intercation clears up a tile so that the move is allowed to succeed?
	fn allows_move(&self) -> bool {
		match self {
			InteractionConsequences::NonLethalHit { .. } => false,
			InteractionConsequences::Kill { .. }
			| InteractionConsequences::Mine
			| InteractionConsequences::StompShroom
			| InteractionConsequences::KeyOpenDoor
			| InteractionConsequences::Heal
			| InteractionConsequences::GainARedo
			| InteractionConsequences::Exit { .. } => true,
		}
	}
}

struct MoveAttemptConsequences {
	/// Will some objects actually move or will they just fail to move?
	success: bool,
	/// The number of object that move or fail to move, not considering what is pulled.
	non_pulled_length: i32,
	/// The number of objects that move by being pulled.
	pulled_length: i32,
	/// The frontmost object to move may interact with an other object in front of it,
	/// if an interaction occurs and its consequences are also consequences of the move.
	final_interaction: Option<InteractionConsequences>,
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
		damages: i32,
	},
	Killed {
		obj: Obj,
		at: IVec2,
		damages: i32,
	},
	Mined {
		obj: Obj,
		at: IVec2,
	},
	DoorOpenedWithKey {
		key_obj: Obj,
		door_obj: Obj,
		from: IVec2,
		to: IVec2,
	},
	Healed {
		obj: Obj,
		at: IVec2,
	},
	RedoGained {
		obj: Obj,
		at: IVec2,
	},
	Exit {
		obj: Obj,
		from: IVec2,
		to: IVec2,
	},
	MoveInto {
		obj: Obj,
		from: IVec2,
		to: IVec2,
	},
	Stomped {
		obj: Obj,
		at: IVec2,
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

impl From<LogicalWorld> for LogicalTransition {
	fn from(lw: LogicalWorld) -> LogicalTransition {
		LogicalTransition { resulting_lw: lw, logical_events: vec![] }
	}
}

impl LogicalTransition {
	pub fn updated_visibility(self) -> LogicalTransition {
		LogicalTransition {
			resulting_lw: self.resulting_lw.updated_visibility(),
			logical_events: self.logical_events,
		}
	}

	pub fn generated_walls_outside(self) -> LogicalTransition {
		LogicalTransition {
			resulting_lw: self.resulting_lw.generated_walls_outside(),
			logical_events: self.logical_events,
		}
	}
}

pub fn four_directions() -> [IVec2; 4] {
	[
		IVec2::from((1, 0)),
		IVec2::from((0, 1)),
		IVec2::from((-1, 0)),
		IVec2::from((0, -1)),
	]
}
