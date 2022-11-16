#![windows_subsystem = "windows"]

mod window;
mod files;
mod vertex_buffer_builder;

extern crate rand;
extern crate winit;
extern crate wgpu;

use std::hint::unreachable_unchecked;
use std::time::{SystemTime, UNIX_EPOCH};
use rand::Rng;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton};
use crate::vertex_buffer_builder::VertexBufferBuilder;
use crate::window::run;

fn main() {
    pollster::block_on(run());
}

pub struct Data {
    mouse_x: f64,
    mouse_y: f64,
    tiles_left: u16,
    placed_mines: bool,
    mines: i16, // this has to be signed
    mouse_held: bool,
    starting_mines: u16,
    death_pos: Option<(usize, usize)>,
    start_time: u64,
    board: Vec<u8>,
    width: usize,
    height: usize,
}

impl Data {
    pub fn new(mines: u16, width: usize, height: usize) -> Data {
        Data {
            mouse_x: 0.0,
            mouse_y: 0.0,
            width,
            height,
            tiles_left: (width * height - mines as usize) as u16,
            placed_mines: false,
            mines: mines as i16,
            mouse_held: false,
            starting_mines: mines,
            death_pos: None,
            start_time: unsafe { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_unchecked() }.as_secs(), // time cannot go backwards
            board: {
                let mut vec = Vec::<u8>::with_capacity(width * height);
                unsafe {
                    vec.as_mut_ptr().write_bytes(0, vec.capacity());
                    vec.set_len(vec.capacity());
                }
                vec
            }
        }
    }

    #[inline(always)]
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline(always)]
    pub fn height(&self) -> usize {
        self.height
    }

    #[inline(always)]
    pub fn get(&self, x: usize, y: usize) -> u8 {
        unsafe { *self.board.get_unchecked(y * self.width + x) }
    }

    #[inline(always)]
    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut u8 {
        unsafe { self.board.get_unchecked_mut(y * self.width + x) }
    }

    pub fn get_uv(&self, x: usize, y: usize) -> (u32, u32) {
        let mouse_x = (self.mouse_x - 12.0) / 16.0;
        let mouse_y = (self.mouse_y - 55.0) / 16.0;
        let tile = self.get(x, y);
        if let Some((death_x, death_y)) = self.death_pos {
            if (tile >> 2) & 3 != 1 && tile & 1 == 1 {
                return (0, 0);
            }
            if (tile >> 2) & 3 == 1 {
                return if x != death_x || y != death_y {
                    (16, 0)
                } else {
                    (0, 16)
                }
            }
        }
        if mouse_x >= 0.0 && mouse_y >= 0.0 && mouse_x as usize == x && mouse_y as usize == y && self.death_pos.is_none() && self.tiles_left != 0 {
            if tile & 1 == 1 {
                return (32, 0);
            }

            if (tile >> 1) & 1 == 0 {
                if self.mouse_held {
                    return (16, 16);
                }
                return (48, 0);
            }
        }
        if tile & 1 == 1 {
            return (32, 16);
        }

        if (tile >> 1) & 1 == 0 {
            return (48, 16);
        }

        return match (tile >> 2) & 3 {
            0 => (16, 16),
            1 => (0, 16),
            _ => match (tile >> 4) & 7 { // check bitmask for unreachable_unchecked assurance
                0 => (48, 48),
                1 => (32, 48),
                2 => (16, 48),
                3 => (0, 48),
                4 => (32, 48),
                5 => (32, 32),
                6 => (32, 16),
                7 => (32, 0),
                _ => unsafe { unreachable_unchecked() } // literally impossible with bitmask
            }
        }
    }

    pub fn clear_board(&mut self) {
        unsafe { self.board.as_mut_ptr().write_bytes(0, self.board.capacity()) }
    }

    pub fn place_mines(&mut self, avoid_x: usize, avoid_y: usize) {
        let mut rand = rand::thread_rng();
        let mut i = 0;
        while i < self.mines {
            let x = rand.gen_range(0..self.width);
            let y = rand.gen_range(0..self.height);
            if x == avoid_x && y == avoid_y {
                continue
            }

            if (self.get(x, y) >> 2) & 3 == 1 {
                continue
            }

            *self.get_mut(x, y) = 0b00000100;

            let width = self.width;
            let height = self.height;
            for &(x, y) in [(x.wrapping_sub(1), y.wrapping_sub(1)), (x, y.wrapping_sub(1)), (x + 1, y.wrapping_sub(1)), (x.wrapping_sub(1), y), (x + 1, y), (x.wrapping_sub(1), y + 1), (x, y + 1), (x + 1, y + 1)].iter().filter(|(x, y)| *x < width && *y < height) {
                match (self.get(x, y) >> 2) & 3 {
                    0 => *self.get_mut(x, y) = 0b00001000,
                    1 => {} // already a mine
                    2 => *self.get_mut(x, y) = ((((self.get(x, y) >> 4) & 7) + 1) << 4) | 0b1000,
                    _ => panic!("Impossible value {} at {} {}", (self.get(x, y) >> 2) & 3, x, y)
                }
            }

            i += 1
        }
    }

    pub fn click(&mut self, x: usize, y: usize) {
        if self.get(x, y) & 3 != 0 {
            return;
        }

        match (self.get(x, y) >> 2) & 3 {
            0 => {
                let mut arr = {
                    let mut vec = Vec::<(usize, usize)>::with_capacity(self.board.capacity());
                    unsafe {
                        vec.as_mut_ptr().write_bytes(0, vec.capacity());
                        vec.set_len(vec.capacity());
                    }
                    vec
                };
                arr[0] = (x, y);
                let mut index = 0;
                let width = self.width;
                let height = self.height;
                loop {
                    let (x, y) = arr[index];
                    if self.get(x, y) & 0b11 == 0 {
                        *self.get_mut(x, y) |= 0b10;
                        self.tiles_left -= 1;
                        for &(x, y) in [(x.wrapping_sub(1), y.wrapping_sub(1)), (x, y.wrapping_sub(1)), (x + 1, y.wrapping_sub(1)), (x.wrapping_sub(1), y), (x + 1, y), (x.wrapping_sub(1), y + 1), (x, y + 1), (x + 1, y + 1)].iter().filter(|(x, y)| *x < width && *y < height) {
                            match (self.get(x, y) >> 2) & 3 {
                                0 => {
                                    arr[index] = (x, y);
                                    index += 1;
                                },
                                1 => panic!("This shouldn't ever happen, a mine next to an empty tile"),
                                2 => {
                                    if self.get(x, y) & 0b10 == 0 {
                                        self.tiles_left -= 1;
                                        *self.get_mut(x, y) |= 0b10;
                                    }
                                },
                                _ => panic!("Impossible value {} at {} {}", (self.get(x, y) >> 2) & 3, x, y)
                            }
                        }
                    }

                    index = index.wrapping_sub(1);
                    if index == usize::MAX {
                        break
                    }
                }
            },
            1 => self.death_pos = Some((x, y)),
            2 => {
                self.tiles_left -= 1u16;
                *self.get_mut(x, y) |= 0b10;
            },
            _ => panic!("Impossible value for tile {:#08b} at {} {}", self.get(x, y), y, x)
        }
    }

    pub fn flag(&mut self, x: usize, y: usize) {
        if self.get(x, y) & 1 != 0 {
            self.mines += 1;
        } else {
            self.mines -= 1;
        }

        *self.get_mut(x, y) ^= 0b1;
    }
}

#[inline]
pub fn render(builder: &mut VertexBufferBuilder, data: &Data) {
    let width: u32 = data.width() as u32;
    let height: u32 = data.height() as u32;
    let reset_x: u32 = (width * 16 - 2) / 2;
    builder.draw_texture((0, 0), (69, 25), (12, 55)); // top left
    for i in 0..width * 16 {
        builder.draw_texture((12 + i, 0), (81, 25), (16, 55)); // top
        builder.draw_texture((12 + i, 55 + height * 16), (81, 96), (16, 8)); // bottom
    }
    for i in 0..height * 16 {
        builder.draw_texture((0, 55 + i), (69, 80), (12, 16)); // left
        builder.draw_texture((12 + width * 16, 55 + i), (97, 80), (8, 16)); // right
    }

    builder.draw_texture((12 + width * 16, 0), (97, 25), (8, 55)); // top right
    builder.draw_texture((0, 55 + height * 16), (69, 96), (12, 8)); // bottom left
    builder.draw_texture((12 + width * 16, 55 + height * 16), (97, 96), (8, 8)); // bottom right

    builder.draw_texture((16, 16), (64, 0), (41, 25)); // mines (left) border
    builder.draw_texture((width * 16 - 35, 16), (64, 0), (41, 25)); // timer (right) border


    for y in 0..height {
        for x in 0..width {
            builder.draw_texture((12 + x * 16, 55 + y * 16), data.get_uv(x as usize, y as usize), (16, 16)); // tile
        }
    }

    if data.mouse_held && data.mouse_x as u32 >= reset_x && reset_x + 26 > data.mouse_x as u32 && data.mouse_y as u32 >= 15 && 41 > data.mouse_y as u32 {
        builder.draw_texture((reset_x, 15), (105, 78), (26, 26)); // pressed
    } else if data.death_pos.is_some() {
        builder.draw_texture((reset_x, 15), (105, 52), (26, 26)); // dead
    } else if data.tiles_left == 0 {
        builder.draw_texture((reset_x, 15), (105, 0), (26, 26)); // sunglasses
    } else {
        builder.draw_texture((reset_x, 15), (105, 26), (26, 26)); // normal
    }

    // mines left
    format!("{:>3}", data.mines).bytes().take(3).map(get_num_uv).enumerate().rev().for_each(|(index, uv)| builder.draw_texture((17 + index as u32 * 13, 17), uv, (13, 23)));

    // seconds right
    let seconds = if data.placed_mines && data.death_pos.is_none() { unsafe { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_unchecked() }.as_secs() - data.start_time } else { 0 };
    format!("{:>3}", seconds).bytes().take(3).map(get_num_uv).enumerate().rev().for_each(|(index, uv)| builder.draw_texture((width * 16 - 34 + index as u32 * 13, 17), uv, (13, 23)));
}

#[inline]
fn get_num_uv(char: u8) -> (u32, u32) {
    match char {
        b'-' => (0, 110),
        b'0' => (52, 87),
        b'1' => (39, 87),
        b'2' => (26, 87),
        b'3' => (13, 87),
        b'4' => (0, 87),
        b'5' => (52, 64),
        b'6' => (39, 64),
        b'7' => (26, 64),
        b'8' => (13, 64),
        b'9' => (0, 64),
        _ => (65, 1)
    }
}

#[inline]
pub fn mouse_click(state: &ElementState, button: &MouseButton, data: &mut Data) {
    let reset_x: u32 = (data.width() as u32 * 16 - 2) / 2;
    match button {
        MouseButton::Left => {
            data.mouse_held = *state == ElementState::Pressed;
            if *state == ElementState::Released {
                let x: i32 = (data.mouse_x - 12.0) as i32;
                let y: i32 = (data.mouse_y - 55.0) as i32;
                if x >= 0 && y >= 0 && data.tiles_left != 0 && data.death_pos.is_none() {
                    let x = (x / 16) as usize;
                    let y = (y / 16) as usize;
                    if x < data.width() && y < data.height() {
                        if !data.placed_mines {
                            data.placed_mines = true;
                            data.start_time = SystemTime::now().duration_since(UNIX_EPOCH).expect("time went backwards lmao").as_secs();
                            data.place_mines(x, y);
                        }
                        data.click(x, y);
                    }
                } else if data.mouse_x as u32 >= reset_x && reset_x + 26 > data.mouse_x as u32 && data.mouse_y as u32 >= 15 && -14 > y {
                    data.death_pos = None;
                    data.placed_mines = false;
                    data.mines = data.starting_mines as i16;
                    data.tiles_left = ((data.width() * data.height()) as i16 - data.mines) as u16;
                    data.clear_board();
                }
            }
        },
        MouseButton::Right => {
            if *state == ElementState::Pressed {
                let x = (data.mouse_x - 12.0) as i32;
                let y = (data.mouse_y - 55.0) as i32;
                if x >= 0 && y >= 0 && data.tiles_left != 0 && data.death_pos.is_none() {
                    let x = x as usize / 16;
                    let y = y as usize / 16;
                    if x < data.width() && y < data.height() && (data.get(x, y) >> 1) & 1 == 0 {
                        data.flag(x, y);
                    }
                }
            }
        },
        MouseButton::Middle => {
            if *state == ElementState::Pressed {
                let x = (data.mouse_x - 12.0) as i32;
                let y = (data.mouse_y - 55.0) as i32;
                if x >= 0 && y >= 0 && data.tiles_left != 0 && data.death_pos.is_none() {
                    let x = x as usize / 16;
                    let y = y as usize / 16;
                    if x < data.width() && y < data.height() {
                        if !data.placed_mines {
                            data.placed_mines = true;
                            data.start_time = SystemTime::now().duration_since(UNIX_EPOCH).expect("time went backwards lmao").as_secs();
                            data.place_mines(x, y);
                        }
                        data.click(x, y);
                        let width = data.width();
                        let height = data.height();
                        for &(x, y) in [(x.wrapping_sub(1), y.wrapping_sub(1)), (x, y.wrapping_sub(1)), (x + 1, y.wrapping_sub(1)), (x.wrapping_sub(1), y), (x + 1, y), (x.wrapping_sub(1), y + 1), (x, y + 1), (x + 1, y + 1)].iter().filter(|(x, y)| *x < width && *y < height) {
                            data.click(x, y);
                        }
                    }
                }
            }
        },
        MouseButton::Other(_) => {}
    }
}

#[inline]
pub fn mouse_moved(position: &PhysicalPosition<f64>, data: &mut Data) {
    data.mouse_x = position.x;
    data.mouse_y = position.y;
}
