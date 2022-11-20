#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![feature(inline_const)]

extern crate rand;
extern crate wgpu;
extern crate winit;

mod assets;
mod vertex_buffer_builder;
mod window;

use crate::vertex_buffer_builder::VertexBufferBuilder;
use crate::window::run;
use rand::Rng;
use window::Theme;
use std::hint::unreachable_unchecked;
use std::time::{SystemTime, UNIX_EPOCH};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, VirtualKeyCode};
use winit::window::Window;

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
    finish_time: Option<u64>,
    resizing: Option<(u32, u32)>,
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
            finish_time: None,
            start_time: unsafe {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_unchecked()
            }
            .as_secs(), // time cannot go backwards
            board: {
                let mut vec = Vec::<u8>::with_capacity(width * height);
                unsafe {
                    vec.as_mut_ptr().write_bytes(0, vec.capacity());
                    vec.set_len(vec.capacity());
                }
                vec
            },
            resizing: None,
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
                };
            }
        }
        if mouse_x >= 0.0
            && mouse_y >= 0.0
            && mouse_x as usize == x
            && mouse_y as usize == y
            && self.death_pos.is_none()
            && self.tiles_left != 0
        {
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
            _ => match (tile >> 4) & 7 {
                // check bitmask for unreachable_unchecked assurance
                0 => (48, 48),
                1 => (32, 48),
                2 => (16, 48),
                3 => (0, 48),
                4 => (32, 48),
                5 => (32, 32),
                6 => (32, 16),
                7 => (32, 0),
                _ => unsafe { unreachable_unchecked() }, // literally impossible with bitmask
            },
        };
    }

    pub fn clear_board(&mut self) {
        unsafe {
            self.board
                .as_mut_ptr()
                .write_bytes(0, self.board.capacity())
        }
    }

    pub fn place_mines(&mut self, avoid_x: usize, avoid_y: usize) {
        let mut rand = rand::thread_rng();
        let mut i = 0;
        while i < self.mines {
            let x = rand.gen_range(0..self.width);
            let y = rand.gen_range(0..self.height);
            if x.wrapping_sub(avoid_x).wrapping_add(1) <= 2
                && y.wrapping_sub(avoid_y).wrapping_add(1) <= 2
            {
                continue;
            }

            if (self.get(x, y) >> 2) & 3 == 1 {
                continue;
            }

            *self.get_mut(x, y) = 0b00000100;

            let width = self.width;
            let height = self.height;
            for &(x, y) in [
                (x.wrapping_sub(1), y.wrapping_sub(1)),
                (x, y.wrapping_sub(1)),
                (x + 1, y.wrapping_sub(1)),
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x.wrapping_sub(1), y + 1),
                (x, y + 1),
                (x + 1, y + 1),
            ]
            .iter()
            .filter(|(x, y)| *x < width && *y < height)
            {
                match (self.get(x, y) >> 2) & 3 {
                    0 => *self.get_mut(x, y) = 0b00001000,
                    1 => {} // already a mine
                    2 => *self.get_mut(x, y) = ((((self.get(x, y) >> 4) & 7) + 1) << 4) | 0b1000,
                    _ => panic!(
                        "Impossible value {} at {} {}",
                        (self.get(x, y) >> 2) & 3,
                        x,
                        y
                    ),
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
                let width = self.width;
                let height = self.height;
                if self.get(x, y) & 0b10 == 0 {
                    *self.get_mut(x, y) |= 0b10;
                    self.tiles_left -= 1;
                    for &(x, y) in [
                        (x.wrapping_sub(1), y.wrapping_sub(1)),
                        (x, y.wrapping_sub(1)),
                        (x + 1, y.wrapping_sub(1)),
                        (x.wrapping_sub(1), y),
                        (x + 1, y),
                        (x.wrapping_sub(1), y + 1),
                        (x, y + 1),
                        (x + 1, y + 1),
                    ]
                    .iter()
                    .filter(|(x, y)| x < &width && y < &height)
                    {
                        if self.get(x, y) & 0b10 == 0 {
                            self.click(x, y)
                        }
                    }
                }
            }
            1 => {
                self.death_pos = Some((x, y));
                self.finish_time = Some(
                    unsafe {
                        std::time::SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_unchecked()
                    }
                    .as_secs()
                        - self.start_time,
                );
            }
            2 => {
                self.tiles_left -= 1;
                *self.get_mut(x, y) |= 0b10;
            }
            _ => panic!(
                "Impossible value for tile {:#08b} at {} {}",
                self.get(x, y),
                y,
                x
            ),
        }

        if self.tiles_left == 0 {
            self.finish_time = Some(
                unsafe {
                    std::time::SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_unchecked()
                }
                .as_secs()
                    - self.start_time,
            );
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
    let width: u32 = data.width as u32;
    let height: u32 = data.height as u32;
    let reset_x: u32 = (width * 16 - 2) / 2;
    {
        let mut remaining = builder.window_width() - 20;
        let mut offset = 12;
        while remaining > 0 {
            builder.draw_texture((offset, 0), (81, 25), (16.min(remaining), 55)); // top
            builder.draw_texture(
                (offset, builder.window_height() - 8),
                (81, 96),
                (16.min(remaining), 8),
            ); // bottom
            if remaining >= 8 {
                remaining -= 8;
                offset += 8;
            } else {
                offset += remaining;
                remaining = 0;
            }
        }
    }
    {
        let mut remaining = builder.window_height() - 63;
        let mut offset = 55;
        while remaining > 0 {
            builder.draw_texture((0, offset), (69, 80), (12, 16.min(remaining))); // left
            builder.draw_texture(
                (builder.window_width() - 8, offset),
                (97, 80),
                (8, 16.min(remaining)),
            ); // right
            if remaining >= 8 {
                remaining -= 8;
                offset += 8;
            } else {
                offset += remaining;
                remaining = 0;
            }
        }
    }

    builder.draw_texture((0, 0), (69, 25), (12, 55)); // top left
    builder.draw_texture((builder.window_width() - 8, 0), (97, 25), (8, 55)); // top right
    builder.draw_texture((0, builder.window_height() - 8), (69, 96), (12, 8)); // bottom left
    builder.draw_texture(
        (builder.window_width() - 8, builder.window_height() - 8),
        (97, 96),
        (8, 8),
    ); // bottom right

    builder.draw_texture((16, 16), (64, 0), (41, 25)); // mines (left) border
    builder.draw_texture((builder.window_width() - 55, 16), (64, 0), (41, 25)); // timer (right) border

    for y in 0..height {
        for x in 0..width {
            builder.draw_texture(
                (12 + x * 16, 55 + y * 16),
                data.get_uv(x as usize, y as usize),
                (16, 16),
            ); // tile
        }
    }

    if data.mouse_held
        && data.mouse_x as u32 >= reset_x
        && reset_x + 26 > data.mouse_x as u32
        && data.mouse_y as u32 >= 15
        && 41 > data.mouse_y as u32
    {
        builder.draw_texture((reset_x, 15), (105, 78), (26, 26)); // pressed
    } else if data.death_pos.is_some() {
        builder.draw_texture((reset_x, 15), (105, 52), (26, 26)); // dead
    } else if data.tiles_left == 0 {
        builder.draw_texture((reset_x, 15), (105, 0), (26, 26)); // sunglasses
    } else {
        builder.draw_texture((reset_x, 15), (105, 26), (26, 26)); // normal
    }

    // mines left
    let mines = if let Some((width, _)) = data.resizing {
        ((width - 20) / 16) as i16
    } else {
        data.mines
    };
    format!("{:>3}", mines)
        .bytes()
        .take(3)
        .map(get_num_uv)
        .enumerate()
        .rev()
        .for_each(|(index, uv)| builder.draw_texture((17 + index as u32 * 13, 17), uv, (13, 23)));

    // seconds right
    let seconds = if let Some((_, height)) = data.resizing {
        ((height - 63) / 16) as u64
    } else {
        if data.placed_mines {
            if let Some(time) = data.finish_time {
                time
            } else {
                unsafe {
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_unchecked()
                }
                .as_secs()
                    - data.start_time
            }
        } else {
            0
        }
    };
    format!("{:>3}", seconds)
        .bytes()
        .take(3)
        .map(get_num_uv)
        .enumerate()
        .rev()
        .for_each(|(index, uv)| {
            builder.draw_texture(
                (builder.window_width() - 54 + index as u32 * 13, 17),
                uv,
                (13, 23),
            )
        });

    // sheen time!!
    if let Some(finish_time) = data.finish_time.map(|x| (x + data.start_time) as u128 * 5) {
        let elapsed = unsafe { std::time::SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_unchecked() }.as_millis() as f64 / 200.0;
        let offset = elapsed - finish_time as f64;
        let offset = if offset >= 6.0 { offset - 6.0 } else { 0.0 };  // estimated time since vsync, overshot because ofc
        let offset = (offset * offset * offset) as u32;
        for x in 0..data.width as u32 {
            for y in 0..data.height as u32 {
                if offset >= x + y + 1 {
                    builder.draw_texture((12 + x * 16, 55 + y * 16), (131, 0), (16, 16));
                }
            }
        }
    }

    //    builder.draw_texture((0, 0), (0, 0), (256, 256));
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
        _ => (65, 1),
    }
}

#[inline]
pub fn mouse_click(
    state: &ElementState,
    button: &MouseButton,
    data: &mut Data,
    window: &mut Window,
    window_state: &mut crate::window::State,
) {
    fit_to_size(data, window, window_state);
    let reset_x: u32 = (data.width as u32 * 16 - 2) / 2;
    match button {
        MouseButton::Left => {
            data.mouse_held = *state == ElementState::Pressed;
            if *state == ElementState::Released {
                let x: i32 = (data.mouse_x - 12.0) as i32;
                let y: i32 = (data.mouse_y - 55.0) as i32;
                if x >= 0 && y >= 0 && data.tiles_left != 0 && data.death_pos.is_none() {
                    let x = (x / 16) as usize;
                    let y = (y / 16) as usize;
                    if x < data.width && y < data.height {
                        if !data.placed_mines {
                            data.placed_mines = true;
                            data.start_time = unsafe {
                                SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_unchecked()
                            }
                            .as_secs();
                            data.place_mines(x, y);
                        }
                        data.click(x, y);
                    }
                } else if data.mouse_x as u32 >= reset_x
                    && reset_x + 26 > data.mouse_x as u32
                    && data.mouse_y as u32 >= 15
                    && -14 > y
                {
                    data.death_pos = None;
                    data.finish_time = None;
                    data.placed_mines = false;
                    data.mines = data.starting_mines as i16;
                    data.tiles_left = ((data.width * data.height) as i16 - data.mines) as u16;
                    data.clear_board();
                }
            }
        }
        MouseButton::Right => {
            if *state == ElementState::Pressed {
                let x = (data.mouse_x - 12.0) as i32;
                let y = (data.mouse_y - 55.0) as i32;
                if x >= 0 && y >= 0 && data.tiles_left != 0 && data.death_pos.is_none() {
                    let x = x as usize / 16;
                    let y = y as usize / 16;
                    if x < data.width && y < data.height && (data.get(x, y) >> 1) & 1 == 0 {
                        data.flag(x, y);
                    }
                }
            }
        }
        MouseButton::Middle => {
            if *state == ElementState::Pressed {
                let x = (data.mouse_x - 12.0) as i32;
                let y = (data.mouse_y - 55.0) as i32;
                if x >= 0 && y >= 0 && data.tiles_left != 0 && data.death_pos.is_none() {
                    let x = x as usize / 16;
                    let y = y as usize / 16;
                    if x < data.width && y < data.height {
                        if !data.placed_mines {
                            data.placed_mines = true;
                            data.start_time = unsafe {
                                SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_unchecked()
                            }
                            .as_secs(); // time cannot go backwards
                            data.place_mines(x, y);
                        }
                        data.click(x, y);
                        let width = data.width;
                        let height = data.height;
                        for &(x, y) in [
                            (x.wrapping_sub(1), y.wrapping_sub(1)),
                            (x, y.wrapping_sub(1)),
                            (x + 1, y.wrapping_sub(1)),
                            (x.wrapping_sub(1), y),
                            (x + 1, y),
                            (x.wrapping_sub(1), y + 1),
                            (x, y + 1),
                            (x + 1, y + 1),
                        ]
                        .iter()
                        .filter(|(x, y)| *x < width && *y < height)
                        {
                            data.click(x, y);
                        }
                    }
                }
            }
        }
        MouseButton::Other(_) => {}
    }
}

#[inline]
pub fn mouse_moved(
    position: &PhysicalPosition<f64>,
    data: &mut Data,
    window: &mut Window,
    state: &mut crate::window::State,
) {
    data.mouse_x = position.x;
    data.mouse_y = position.y;
    fit_to_size(data, window, state);
}

pub fn fit_to_size(data: &mut Data, window: &mut Window, state: &mut crate::window::State) {
    if let Some((width, height)) = data.resizing.take() {
        let width = ((width - 20) & !0b1111) + 20;
        let height = ((height - 63) & !0b1111) + 63;
        window.set_inner_size(PhysicalSize::new(width, height));
        state.resize(PhysicalSize::new(width, height));
    }
}

#[inline]
pub fn key_input(
    input: winit::event::KeyboardInput,
    data: &mut Data,
    window: &mut Window,
    state: &mut crate::window::State,
) {
    fit_to_size(data, window, state);
    if input.state == ElementState::Released {
        if let Some(x) = input.virtual_keycode {
            if x == VirtualKeyCode::B {
                data.width = 9;
                data.height = 9;
                data.placed_mines = false;
                data.starting_mines = 10;
                data.mouse_x = 0.0;
                data.mouse_y = 0.0;
                data.mines = 10;
                data.tiles_left = (9 * 9) - 10;
                data.death_pos = None;
                data.finish_time = None;
                data.board = {
                    let mut vec = Vec::<u8>::with_capacity(9 * 9);
                    unsafe {
                        vec.as_mut_ptr().write_bytes(0, 9 * 9);
                        vec.set_len(9 * 9);
                    }
                    vec
                };
                let size = PhysicalSize::new((20 + 16 * 9) as u32, (63 + 16 * 9) as u32);
                window.set_inner_size(size.clone());
                state.resize(size);
            } else if x == VirtualKeyCode::I {
                data.width = 16;
                data.height = 16;
                data.placed_mines = false;
                data.starting_mines = 40;
                data.mouse_x = 0.0;
                data.mouse_y = 0.0;
                data.mines = 40;
                data.tiles_left = (16 * 16) - 40;
                data.death_pos = None;
                data.finish_time = None;
                data.board = {
                    let mut vec = Vec::<u8>::with_capacity(16 * 16);
                    unsafe {
                        vec.as_mut_ptr().write_bytes(0, 16 * 16);
                        vec.set_len(16 * 16);
                    }
                    vec
                };
                let size = PhysicalSize::new((20 + 16 * 16) as u32, (63 + 16 * 16) as u32);
                window.set_inner_size(size.clone());
                state.resize(size);
            } else if x == VirtualKeyCode::E {
                data.width = 30;
                data.height = 16;
                data.placed_mines = false;
                data.starting_mines = 99;
                data.mouse_x = 0.0;
                data.mouse_y = 0.0;
                data.mines = 99;
                data.tiles_left = (30 * 16) - 99;
                data.death_pos = None;
                data.finish_time = None;
                data.board = {
                    let mut vec = Vec::<u8>::with_capacity(30 * 16);
                    unsafe {
                        vec.as_mut_ptr().write_bytes(0, 30 * 16);
                        vec.set_len(30 * 16);
                    }
                    vec
                };
                let size = PhysicalSize::new((20 + 16 * 30) as u32, (63 + 16 * 16) as u32);
                window.set_inner_size(size.clone());
                state.resize(size);
            } else if x == VirtualKeyCode::Up
                && data.starting_mines as usize + 9 < data.width * data.height
            {
                data.placed_mines = false;
                data.mouse_x = 0.0;
                data.mouse_y = 0.0;
                data.starting_mines += 1;
                data.mines = data.starting_mines as i16;
                data.tiles_left = (data.width * data.height) as u16 - data.starting_mines;
                data.death_pos = None;
                data.finish_time = None;
                data.clear_board(); // keep size
            } else if x == VirtualKeyCode::Down && data.starting_mines > 0 {
                data.placed_mines = false;
                data.mouse_x = 0.0;
                data.mouse_y = 0.0;
                data.starting_mines -= 1;
                data.mines = data.starting_mines as i16;
                data.tiles_left = (data.width * data.height) as u16 - data.starting_mines;
                data.death_pos = None;
                data.finish_time = None;
                data.clear_board(); // keep size
            } else if x == VirtualKeyCode::L {
                state.theme = Theme::Light;
            } else if x == VirtualKeyCode::D {
                state.theme = Theme::Dark;
            }
        }
    }
}

pub fn on_resize(size: PhysicalSize<u32>, data: &mut Data) {
    data.resizing = Some((size.width, size.height));
    data.width = (size.width as usize - 20) / 16;
    data.height = (size.height as usize - 63) / 16;
    data.placed_mines = false;
    data.mines = data.starting_mines as i16;
    data.tiles_left = (data.width * data.height) as u16 - data.starting_mines;
    data.death_pos = None;
    data.finish_time = None;
    data.board = {
        let mut vec = Vec::<u8>::with_capacity(data.width * data.height);
        unsafe {
            vec.as_mut_ptr().write_bytes(0, vec.capacity());
            vec.set_len(vec.capacity());
        }
        vec
    }
}
