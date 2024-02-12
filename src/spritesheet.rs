//! Spritesheet related matters, such as loading or providing the rect of a sprite in the sheet.

use ggez::{
	glam::IVec2,
	graphics::{Image, ImageFormat, Rect},
	Context, GameResult,
};
use image::EncodableLayout;

pub struct SpritesheetStuff {
	pub spritesheet: Image,
	/// Used as a mask to multiply it by a color for like hit effect red blinking.
	pub spritesheet_white: Image,
}

impl SpritesheetStuff {
	pub fn new(ctx: &mut Context) -> GameResult<SpritesheetStuff> {
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

/// These refer to a sprite in the spritesheet.
pub enum SpriteFromSheet {
	Wall,
	Floor,
	Sword,
	Shield,
	Rock,
	Bunny,
	Slime,
	Pickaxe,
	Exit,
	VisionGem,
	Key,
	Door,
	Rope,
	Shroomer,
	Shroom,
	Bush,
	Heart,
	RedoHeart,
	Fish(IVec2),
	Digit(u8),
	Slash,
}

impl SpriteFromSheet {
	pub fn rect_in_spritesheet(&self) -> Rect {
		// Wild non-aligned sprites.
		if let SpriteFromSheet::Digit(digit) = self {
			let x = digit * 4;
			let y = 16;
			return Rect::new(x as f32 / 128.0, y as f32 / 128.0, 3.0 / 128.0, 5.0 / 128.0);
		} else if let SpriteFromSheet::Slash = self {
			let x = 10 * 4;
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
			SpriteFromSheet::Pickaxe => (6, 0),
			SpriteFromSheet::Exit => (7, 0),
			SpriteFromSheet::VisionGem => (8, 0),
			SpriteFromSheet::Key => (9, 0),
			SpriteFromSheet::Door => (10, 0),
			SpriteFromSheet::Rope => (11, 0),
			SpriteFromSheet::Shroomer => (12, 0),
			SpriteFromSheet::Shroom => (13, 0),
			SpriteFromSheet::Bush => (14, 0),
			SpriteFromSheet::Heart => (1, 1),
			SpriteFromSheet::RedoHeart => (2, 1),
			SpriteFromSheet::Fish(IVec2 { x: -1, y: 0 }) => (3, 1),
			SpriteFromSheet::Fish(IVec2 { x: 1, y: 0 }) => (4, 1),
			SpriteFromSheet::Fish(IVec2 { x: 0, y: -1 }) => (5, 1),
			SpriteFromSheet::Fish(IVec2 { x: 0, y: 1 }) => (6, 1),
			SpriteFromSheet::Fish(invalid_direction) => {
				panic!("direction {invalid_direction} is not a valid fish direction")
			},
			SpriteFromSheet::Digit(_) | SpriteFromSheet::Slash => unreachable!("Handled above"),
		};
		Rect::new(
			x as f32 * 8.0 / 128.0,
			y as f32 * 8.0 / 128.0,
			8.0 / 128.0,
			8.0 / 128.0,
		)
	}
}
