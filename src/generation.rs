//! Procedural generation of levels.

use ggez::glam::IVec2;
use rand::{thread_rng, Rng};

use crate::gameplay::{LogicalWorld, Obj, Tile};

fn randint(inf: i32, sup_included: i32) -> i32 {
	thread_rng().gen_range(inf..=sup_included)
}

pub fn filled_rect(top_left: IVec2, dimensions: IVec2) -> Vec<IVec2> {
	let mut vec = vec![];
	for y in top_left.y..(top_left.y + dimensions.y) {
		for x in top_left.x..(top_left.x + dimensions.x) {
			vec.push(IVec2::new(x, y));
		}
	}
	vec
}

fn filled_inner_rect(top_left: IVec2, dimensions: IVec2) -> Vec<IVec2> {
	filled_rect(top_left + IVec2::new(1, 1), dimensions - IVec2::new(2, 2))
}

fn line_rect(top_left: IVec2, dimensions: IVec2) -> Vec<IVec2> {
	let mut outer_vec = filled_rect(top_left, dimensions);
	let inner_vec = filled_inner_rect(top_left, dimensions);
	outer_vec.retain(|coords| !inner_vec.contains(coords));
	outer_vec
}

struct Generator {
	lw: LogicalWorld,
}

impl Generator {
	fn new() -> Generator {
		Generator { lw: LogicalWorld::new_empty() }
	}

	fn generate_empty_room(&mut self, top_left: IVec2, dimensions: IVec2) {
		for coords in line_rect(top_left, dimensions) {
			self.lw.place_tile_no_overwrite(coords, Tile::obj(Obj::Wall));
		}
		for coords in filled_rect(top_left, dimensions) {
			self.lw.place_tile_no_overwrite(coords, Tile::floor());
		}
	}

	fn generate_corridor(&mut self, start: IVec2, direction: IVec2, length: i32, width: i32) {
		let mut coords = start;
		for _ in 0..length {
			let one_wall = coords + direction.perp();
			self.lw.place_tile_no_overwrite(one_wall, Tile::obj(Obj::Wall));
			self.lw.place_tile_no_overwrite(
				one_wall - direction.perp() * (width + 1),
				Tile::obj(Obj::Wall),
			);
			for i in 1..=width {
				self.lw.place_tile(one_wall - direction.perp() * i, Tile::floor());
			}
			coords += direction;
		}
	}

	fn generate_corridor_then_room(&mut self, start: IVec2, direction: IVec2, propagation: i32) {
		let already_room_forward = 'already: {
			for i in 1..20 {
				if self.lw.tile(start + direction * i).is_some() {
					break 'already true;
				}
			}
			false
		};
		if already_room_forward {
			let mut coords = start;
			loop {
				let end = self.lw.tile(coords + direction).is_some();
				self.generate_corridor(coords, direction, 2, 1);
				self.lw.place_tile(coords, Tile::floor());
				if end {
					break;
				}
				coords += direction;
			}
			return;
		}

		let corridor_length = randint(1, 3);
		self.generate_corridor(start, direction, corridor_length, 1);
		let room_entry = start + direction * corridor_length;
		let measure_forward = randint(4, 9);
		let measure_perpward = randint(2, 5);
		let measure_minusperpward = randint(2, 5);
		let in_room_back = room_entry + direction * measure_forward;
		let in_room_perp = room_entry + direction.perp() * measure_perpward;
		let in_room_minusperp = room_entry - direction.perp() * measure_minusperpward;
		let top_left = in_room_back.min(in_room_perp).min(in_room_minusperp);
		let bottom_right = in_room_back.max(in_room_perp).max(in_room_minusperp);
		let dimensions = bottom_right - top_left + IVec2::new(1, 1);

		self.generate_empty_room(top_left, dimensions);
		self.lw.place_tile(room_entry, Tile::floor());

		if propagation >= 1 {
			if randint(0, 1) == 0 {
				let exit = in_room_perp + direction * randint(1, measure_forward - 1);
				self.generate_corridor_then_room(exit, direction.perp(), propagation - 1);
			}
			if randint(0, 1) == 0 {
				let exit = in_room_perp + direction * randint(1, measure_forward - 1);
				self.generate_corridor_then_room(exit, -direction.perp(), propagation - 1);
			}
			if randint(0, 2) == 0 {
				self.generate_corridor_then_room(in_room_back, direction, propagation - 1);
			}
		}
	}

	fn generate_level(&mut self) {
		// Starting room.
		self.generate_empty_room(IVec2::new(-4, -4), IVec2::new(9, 9));
		self.lw.place_tile(IVec2::new(0, 0), Tile::obj(Obj::Bunny { hp: 5, max_hp: 5 }));
		self.lw.place_tile(IVec2::new(-2, 0), Tile::obj(Obj::Shield));
		self.lw.place_tile(IVec2::new(2, 0), Tile::obj(Obj::Sword));
		self.generate_corridor(IVec2::new(4, 0), IVec2::new(1, 0), 4, 1);

		// Test.
		self.lw.place_tile(IVec2::new(0, -2), Tile::obj(Obj::VisionGem));
		self.generate_corridor_then_room(IVec2::new(-4, 0), IVec2::new(-1, 0), 8);

		// Succession of rooms.
		let mut room_x = 8;
		let mut entry_y = 0;
		let room_count = 6;
		for room_index in 0..room_count {
			let is_last_room = room_index == room_count - 1;

			// Room dimensions.
			let up = randint(1, 5) + 2;
			let down = randint(1, 5) + 2;
			let right = randint(2, 8) + 2;
			let top_left = IVec2::new(room_x, entry_y - up);
			let dimensions = IVec2::new(right, up + 1 + down);
			self.generate_empty_room(top_left, dimensions);
			self.lw.place_tile(IVec2::new(room_x, entry_y), Tile::floor());

			// Weighted table of object spawn.
			let obj_table = [
				(100, None),
				(5, Some(Obj::Rock)),
				(1, Some(Obj::Sword)),
				(1, Some(Obj::Shield)),
				(1, Some(Obj::Pickaxe)),
				(1, Some(Obj::VisionGem)),
				(5, Some(Obj::Slime { hp: 5, move_token: false })),
			];
			let total_weight: i32 = obj_table.iter().map(|(weight, _obj)| weight).sum();
			// Fill the room.
			for coords in filled_inner_rect(top_left, dimensions) {
				let mut random_value = randint(0, total_weight - 1);
				let obj = 'obj: {
					for weighted_obj in obj_table.iter() {
						let (weight, obj) = weighted_obj;
						random_value -= weight;
						if random_value < 0 {
							break 'obj obj;
						}
					}
					unreachable!("The value should reach zero before the end due to the range");
				};
				if let Some(obj) = obj {
					self.lw.place_tile(coords, Tile::obj(obj.clone()));
				}
			}

			if randint(0, 6 - 1) == 0 {
				let v = randint(2, 4);
				for coords in filled_inner_rect(top_left, dimensions) {
					if ((coords.x + coords.y) % v == 0 && coords.x % 2 == 0 && randint(0, 6 - 1) != 0)
						|| ((coords.x + coords.y) % 2 != v && randint(0, 10 - 1) == 0)
					{
						self.lw.place_tile(coords, Tile::obj(Obj::Wall));
					}
				}
			}

			if is_last_room {
				// Exit.
				let x = top_left.x + randint(0, dimensions.x - 1);
				let y = top_left.y + randint(0, dimensions.y - 1);
				let coords = IVec2::new(x, y);
				self.lw.place_tile(coords, Tile::obj(Obj::Exit));
			}

			if !is_last_room {
				// Exit corridor to next room.
				let exit_x = room_x + right - 1;
				let exit_y = randint(top_left.y + 1, (top_left.y + dimensions.y - 1) - 1);
				let corridor_length = randint(1, 4);
				self.generate_corridor(
					IVec2::new(exit_x, exit_y),
					IVec2::new(1, 0),
					corridor_length,
					2,
				);
				room_x = exit_x + corridor_length - 1;
				entry_y = exit_y;
			}
		}
	}
}

pub fn generate_level() -> LogicalWorld {
	let mut generator = Generator::new();
	generator.generate_level();
	generator.lw
}
