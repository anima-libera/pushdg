//! Procedural generation of levels.

use ggez::glam::IVec2;
use rand::{thread_rng, Rng};

use crate::gameplay::{LogicalWorld, Obj, Tile};

fn filled_rect(top_left: IVec2, dimensions: IVec2) -> Vec<IVec2> {
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
			let one_wall = coords + direction.perp() * (width / 2 + 1);
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

	fn generate_level(&mut self) {
		// Starting room.
		self.generate_empty_room(IVec2::new(-4, -4), IVec2::new(9, 9));
		self.lw.place_tile(IVec2::new(0, 0), Tile::obj(Obj::Bunny { hp: 5 }));
		self.lw.place_tile(IVec2::new(-2, 0), Tile::obj(Obj::Shield));
		self.lw.place_tile(IVec2::new(2, 0), Tile::obj(Obj::Sword));
		self.generate_corridor(IVec2::new(4, 0), IVec2::new(1, 0), 4, 1);

		// Succession of rooms.
		let mut room_x = 8;
		let mut entry_y = 0;
		for _ in 0..6 {
			// Room dimensions.
			let up = thread_rng().gen_range(1..=5) + 2;
			let down = thread_rng().gen_range(1..=5) + 2;
			let right = thread_rng().gen_range(1..=8) + 2;
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
				(5, Some(Obj::Slime { hp: 5, move_token: false })),
			];
			let total_weight: i32 = obj_table.iter().map(|(weight, _obj)| weight).sum();
			// Fill the room.
			for coords in filled_inner_rect(top_left, dimensions) {
				let mut random_value = thread_rng().gen_range(0..total_weight);
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

			// Exit corridor to next room.
			let exit_x = room_x + right - 1;
			let exit_y = thread_rng().gen_range((top_left.y + 1)..(top_left.y + dimensions.y - 1));
			let corridor_length = thread_rng().gen_range(1..=4);
			self.generate_corridor(
				IVec2::new(exit_x, exit_y),
				IVec2::new(1, 0),
				corridor_length,
				2,
			);
			room_x = exit_x + corridor_length;
			entry_y = exit_y;
		}
	}
}

pub fn generate_level() -> LogicalWorld {
	let mut generator = Generator::new();
	generator.generate_level();
	generator.lw
}
