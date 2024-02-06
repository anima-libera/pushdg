//! Procedural generation of levels.

use ggez::glam::IVec2;
use rand::{seq::SliceRandom, thread_rng, Rng};

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
				let coords = one_wall - direction.perp() * i;
				if self
					.lw
					.tile(coords)
					.is_some_and(|tile| tile.obj.as_ref().is_some_and(|obj| matches!(obj, Obj::Wall)))
				{
					self.lw.place_tile(coords, Tile::floor());
				} else {
					self.lw.place_tile_no_overwrite(coords, Tile::floor());
				}
			}
			coords += direction;
		}
	}

	fn generate_grid_room(&mut self, room_grid_coords: IVec2, is_exit_room: bool) {
		let dimensions = IVec2::new(9, 9);
		let space = IVec2::new(1, 1);
		let top_left = room_grid_coords * (dimensions + space);
		self.generate_empty_room(top_left, dimensions);

		let is_starting_room = room_grid_coords == IVec2::new(0, 0);
		if is_starting_room {
			self.lw.place_tile(
				top_left + dimensions / 2,
				Tile::obj(Obj::Bunny { hp: 7, max_hp: 7 }),
			);
			self.lw.place_tile(
				top_left + dimensions / 2 + IVec2::new(-2, 0),
				Tile::obj(Obj::Shield),
			);
			self.lw.place_tile(
				top_left + dimensions / 2 + IVec2::new(2, 0),
				Tile::obj(Obj::Sword),
			);
		} else {
			// Weighted table of object spawn.
			let obj_table = [
				(500, None),
				(25, Some(Obj::Rock)),
				(5, Some(Obj::Sword)),
				(4, Some(Obj::Shield)),
				(2, Some(Obj::Pickaxe)),
				(3, Some(Obj::VisionGem)),
				(1, Some(Obj::Heart)),
				(2, Some(Obj::RedoHeart)),
				(3, Some(Obj::Key)),
				(3, Some(Obj::Rope)),
				(25, Some(Obj::Slime { hp: 5, move_token: false })),
				(10, Some(Obj::Shroomer { hp: 5, move_token: false })),
				(6, Some(Obj::Shroom)),
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

			if randint(0, 3) == 0 {
				let v = randint(2, 4);
				for coords in filled_inner_rect(top_left, dimensions) {
					if ((coords.x + coords.y) % v == 0 && coords.x % 2 == 0 && randint(0, 6 - 1) != 0)
						|| ((coords.x + coords.y) % 2 != v && randint(0, 10 - 1) == 0)
					{
						let wall = if randint(0, 30) == 0 {
							Obj::Door
						} else {
							Obj::Wall
						};
						self.lw.place_tile(coords, Tile::obj(wall));
					}
				}
			}
		}

		if is_exit_room {
			// Exit.
			let x = top_left.x + randint(0, dimensions.x - 1);
			let y = top_left.y + randint(0, dimensions.y - 1);
			let coords = IVec2::new(x, y);
			self.lw.place_tile(coords, Tile::obj(Obj::Exit));
		}
	}

	fn generate_grid_corridor(&mut self, room_grid_coords: IVec2, direction: IVec2) {
		let dimensions = IVec2::new(9, 9);
		let space = IVec2::new(1, 1);
		let top_left = room_grid_coords * (dimensions + space);
		let center = top_left + dimensions / 2;
		let number_of_corridors = if randint(0, 4) == 0 {
			0
		} else if randint(0, 3) == 0 {
			randint(2, 6)
		} else {
			1
		};
		for _ in 0..number_of_corridors {
			let start = center + direction.perp() * randint(-dimensions.x / 2, dimensions.x / 2);
			self.generate_corridor(start, direction, (dimensions + space).x, 1);
			if number_of_corridors == 1 && randint(0, 3) == 0 {
				let coords = start + direction * ((dimensions + space).x / 2);
				self.lw.place_tile(coords, Tile::obj(Obj::Door));
			}
		}
	}

	fn generate_level(&mut self) {
		// Grid layout.
		let grid_w_radius = 3;
		let grid_h_radius = 3;
		let grid_w = grid_w_radius * 2 + 1;
		let grid_h = grid_h_radius * 2 + 1;
		let grid_x_inf = -grid_w_radius;
		let grid_x_sup = grid_w_radius;
		let grid_y_inf = -grid_h_radius;
		let grid_y_sup = grid_h_radius;
		let exit_rooms: Vec<_> = line_rect(
			IVec2::new(grid_x_inf, grid_y_inf),
			IVec2::new(grid_w, grid_h),
		)
		.choose_multiple(&mut thread_rng(), 3)
		.copied()
		.collect();
		for grid_y in grid_y_inf..=grid_y_sup {
			for grid_x in grid_x_inf..=grid_x_sup {
				let room_grid_coords = IVec2::new(grid_x, grid_y);
				self.generate_grid_room(room_grid_coords, exit_rooms.contains(&room_grid_coords));
			}
		}
		for grid_y in grid_y_inf..=grid_y_sup {
			for grid_x in grid_x_inf..=grid_x_sup {
				let room_grid_coords = IVec2::new(grid_x, grid_y);
				if grid_x < grid_x_sup {
					self.generate_grid_corridor(room_grid_coords, IVec2::new(1, 0));
				}
				if grid_y < grid_y_sup {
					self.generate_grid_corridor(room_grid_coords, IVec2::new(0, 1));
				}
			}
		}
	}
}

pub fn generate_level() -> LogicalWorld {
	let mut generator = Generator::new();
	generator.generate_level();
	generator.lw
}
