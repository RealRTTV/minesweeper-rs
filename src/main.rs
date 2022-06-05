mod window;
mod files;
mod drawable_helper;

extern crate rand;
extern crate winit;
extern crate env_logger;
extern crate log;
extern crate wgpu;
extern crate winapi;
extern crate mex_sys;
extern crate libc;
extern crate bytemuck;

use std::time::{SystemTime, UNIX_EPOCH};
use rand::{Rng, thread_rng};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{DeviceId, ElementState, MouseButton};
use crate::drawable_helper::draw_texture;
use crate::window::{run, Vertex};

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
    dead: bool,
    starting_mines: u16,
    death_x: i32,
    death_y: i32,
    start_time: u64
}

impl Data {
    pub fn new(tiles_left: u16, mines: u16) -> Data {
        Data {
            mouse_x: 0.0,
            mouse_y: 0.0,
            tiles_left,
            placed_mines: false,
            mines: mines as i16,
            mouse_held: false,
            dead: false,
            starting_mines: mines,
            death_x: -1,
            death_y: -1,
            start_time: SystemTime::now().duration_since(UNIX_EPOCH).expect("time went backwards lmao").as_secs()
        }
    }
}

#[inline]
fn flag(x: usize, y: usize, board: &mut Vec<Vec<u8>>, mines: &mut i16) {
    if board[y][x] & 1 != 0 {
        *mines += 1;
    } else {
        *mines -= 1;
    }

    board[y][x] = board[y][x] ^ 1;
}

#[inline]
fn click_all_adj(board: &mut Vec<Vec<u8>>, x: usize, y: usize, data: &mut Data, func: fn(usize, usize, &mut Vec<Vec<u8>>, &mut Data)) {
    let up = y != 0;
    let down = board.len() -1 != y;
    let left = x != 0;
    let right = board[y].len() - 1 != x;
    if up { // up
        func(x, y - 1, board, data);
    }

    if down { // down
        func(x, y + 1, board, data);
    }

    if left { // left
        func(x - 1, y, board, data);
    }

    if right { // right
        func(x + 1, y, board, data);
    }

    if up && left { // up + left
        func(x - 1, y - 1, board, data);
    }

    if up && right { // up + right
        func(x + 1, y - 1, board, data);
    }

    if down && left { // down + left
        func(x - 1, y + 1, board, data);
    }

    if down && right { // down + right
        func(x + 1, y + 1, board, data);
    }
}

// can inline because inline doesn't inline the recursive method calls, only the first one
#[inline]
fn click(x: usize, y: usize, board: &mut Vec<Vec<u8>>, data: &mut Data) {
    if (board[y][x] >> 1) & 1 != 0 || board[y][x] & 1 != 0 {
        return;
    }
    board[y][x] = board[y][x] | 0b10;
    match (board[y][x] >> 2) & 3 {
        0 => {
            click_all_adj(board, x, y, data, click);
            data.tiles_left -= 1u16;
        }
        1 => {
            data.dead = true;
            data.death_x = x as i32;
            data.death_y = y as i32;
        }
        2 => {
            data.tiles_left -= 1u16;
        }
        _ => panic!("Impossible value for tile {:#08b} at {} {}", board[y][x], y, x)
    }
}

#[inline]
fn place_mines(board: &mut Vec<Vec<u8>>, x: usize, y: usize, count: u16, data: &mut Data) {
    let mut random = thread_rng();
    for _ in 0..count {
        loop {
            let mine_x = random.gen_range(0..board[y].len());
            let mine_y = random.gen_range(0..board.len());
            if mine_y == y && mine_x == x || (board[mine_y][mine_x] >> 2) & 3 == 1 {
                continue;
            } else {
                board[mine_y][mine_x] = 0b00000100u8;
                click_all_adj(board, mine_x, mine_y, data, |x, y, board, _| {
                    match (board[y][x] >> 2) & 3 {
                        0 => {
                            board[y][x] = 0b00001000u8;
                        },
                        1 => {
                            // do nothing
                        }
                        2 => {
                            board[y][x] = ((((board[y][x] >> 4) & 7) + 1) << 4) | 0b1000;
                        }
                        _ => panic!("Impossible value {} at {} {}", (board[y][x] >> 2) & 3, x, y)
                    }
                });
                break
            }
        }
    }
}

#[inline]
fn get_uv(tile: &u8, x: usize, y: usize, data: &Data) -> (u32, u32) {
    let mouse_x: f64 = (data.mouse_x - 12.0) / 16.0;
    let mouse_y: f64 = (data.mouse_y - 55.0) / 16.0;
    if data.dead {
        if (tile >> 2) & 3 != 1 && tile & 1 == 1 {
            return (0, 0);
        }
        if (tile >> 2) & 3 == 1 && (x as i32 != data.death_x || y as i32 != data.death_y) {
            return (16, 0);
        }
    }
    if mouse_x >= 0.0 && mouse_y >= 0.0 && mouse_x as usize == x && mouse_y as usize == y && !data.dead && data.tiles_left != 0 {
        if tile & 1 == 1 {
            return (32, 0);
        }

        if (tile >> 1) & 1 == 0 {
            if data.mouse_held {
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
            0 => (48, 48),
            1 => (32, 48),
            2 => (16, 48),
            3 => (0, 48),
            4 => (32, 48),
            5 => (32, 32),
            6 => (32, 16),
            7 => (32, 0),
            _ => panic!("Impossible warning tile value {} at {} {}", tile >> 4 & 7, mouse_x, mouse_y)
        }
    }
}

#[inline]
pub fn render(vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>, size: &PhysicalSize<u32>, board: &mut Vec<Vec<u8>>, data: &Data) {
    let width: u32 = board[0].len() as u32;
    let height: u32 = board.len() as u32;
    let reset_x: u32 = (width * 16 - 2) / 2;
    draw_texture(vertices, indices, size, 0, 0, 69, 25, 12, 55); // top left
    for x in 0..width {
        draw_texture(vertices, indices, size, 12 + x * 16, 0, 81, 25, 16, 55); // top
        draw_texture(vertices, indices, size, 12 + x * 16, 55 + height * 16, 81, 96, 16, 8); // bottom
    }
    for y in 0..height {
        draw_texture(vertices, indices, size, 0, 55 + y * 16, 69, 80, 12, 16); // left
        draw_texture(vertices, indices, size, 12 + width * 16, 55 + y * 16, 97, 80, 8, 16); // right

        for x in 0..width {
            let (u, v) = get_uv(&board[y as usize][x as usize], x as usize, y as usize, data);
            // todo, make efficient to not draw 480 vertices
            draw_texture(vertices, indices, size, 12 + x * 16, 55 + y * 16, u, v, 16, 16); // tile
        }
    }
    draw_texture(vertices, indices, size, 12 + width * 16, 0, 97, 25, 8, 55); // top right
    draw_texture(vertices, indices, size, 0, 55 + height * 16, 69, 96, 12, 8); // bottom left
    draw_texture(vertices, indices, size, 12 + width * 16, 55 + height * 16, 97, 96, 8, 8); // bottom right

    draw_texture(vertices, indices, size, 16, 16, 64, 0, 41, 25); // mines (left) border
    draw_texture(vertices, indices, size, width * 16 - 35, 16, 64, 0, 41, 25); // timer (right) border

    if data.mouse_held && data.mouse_x as u32 >= reset_x && reset_x + 26 > data.mouse_x as u32 && data.mouse_y as u32 >= 15 && 41 > data.mouse_y as u32 {
        draw_texture(vertices, indices, size, reset_x, 15, 105, 78, 26, 26); // pressed
    } else if data.dead {
        draw_texture(vertices, indices, size, reset_x, 15, 105, 52, 26, 26); // dead
    } else if data.tiles_left == 0 {
        draw_texture(vertices, indices, size, reset_x, 15, 105, 0, 26, 26); // sunglasses
    } else {
        draw_texture(vertices, indices, size, reset_x, 15, 105, 26, 26, 26); // normal
    }

    // mines left
    let mines_bytes = &mut format!("{:>3}", data.mines).into_bytes();
    while mines_bytes.len() > 3 { // todo, how faster, this is slow af
        mines_bytes.reverse();
        mines_bytes.pop();
        mines_bytes.reverse();
    }
    let (u1, v1) = get_num_uv(mines_bytes[0]);
    let (u2, v2) = get_num_uv(mines_bytes[1]);
    let (u3, v3) = get_num_uv(mines_bytes[2]);
    draw_texture(vertices, indices, size, 17, 17, u1, v1, 13, 23);
    draw_texture(vertices, indices, size, 30, 17, u2, v2, 13, 23);
    draw_texture(vertices, indices, size, 43, 17, u3, v3, 13, 23);

    // seconds right
    let seconds = if data.placed_mines && !data.dead {SystemTime::now().duration_since(UNIX_EPOCH).expect("time went backwards lmao").as_secs() - data.start_time} else {0u64};
    let mines_bytes = &mut format!("{:>3}", seconds).into_bytes();
    while mines_bytes.len() > 3 { // todo, how faster, this is slow af
        mines_bytes.reverse();
        mines_bytes.pop();
        mines_bytes.reverse();
    }
    let (u1, v1) = get_num_uv(mines_bytes[0]);
    let (u2, v2) = get_num_uv(mines_bytes[1]);
    let (u3, v3) = get_num_uv(mines_bytes[2]);
    draw_texture(vertices, indices, size, width * 16 - 34, 17, u1, v1, 13, 23);
    draw_texture(vertices, indices, size, width * 16 - 21, 17, u2, v2, 13, 23);
    draw_texture(vertices, indices, size, width * 16 - 8, 17, u3, v3, 13, 23);
}

#[inline]
fn get_num_uv(char: u8) -> (u32, u32) {
    match char {
        45 => (0, 110),
        48 => (52, 87),
        49 => (39, 87),
        50 => (26, 87),
        51 => (13, 87),
        52 => (0, 87),
        53 => (52, 64),
        54 => (39, 64),
        55 => (26, 64),
        56 => (13, 64),
        57 => (0, 64),
        _ => (65, 1)
    }
}

#[inline]
pub fn mouse_click(_device_id: &DeviceId, state: &ElementState, button: &MouseButton, data: &mut Data, board: &mut Vec<Vec<u8>>) -> bool {
    let reset_x: u32 = (board[0].len() as u32 * 16 - 2) / 2;
    match button {
        MouseButton::Left => {
            data.mouse_held = *state == ElementState::Pressed;
            if *state == ElementState::Released {
                let mut x: i32 = (data.mouse_x - 12.0) as i32;
                let mut y: i32 = (data.mouse_y - 55.0) as i32;
                if x >= 0 && y >= 0 && data.tiles_left != 0 && !data.dead {
                    x = x / 16;
                    y = y / 16;
                    if x < board[0].len() as i32 && y < board.len() as i32 {
                        if !data.placed_mines {
                            data.placed_mines = true;
                            data.start_time = SystemTime::now().duration_since(UNIX_EPOCH).expect("time went backwards lmao").as_secs();
                            place_mines(board, x as usize, y as usize, data.mines as u16, data);
                        }
                        click(x as usize, y as usize, board, data);
                    }
                } else if data.mouse_x as u32 >= reset_x && reset_x + 26 > data.mouse_x as u32 && data.mouse_y as u32 >= 15 && -14 > y as i32 {
                    data.dead = false;
                    data.placed_mines = false;
                    data.mines = data.starting_mines as i16;
                    data.tiles_left = ((board.len() * board[0].len()) as i16 - data.mines) as u16;
                    *board = vec!(vec!(0u8; board[0].len()); board.len());
                }
            }
        },
        MouseButton::Right => {
            if *state == ElementState::Pressed {
                let mut x: i32 = (data.mouse_x - 12.0) as i32;
                let mut y: i32 = (data.mouse_y - 55.0) as i32;
                if x >= 0 && y >= 0 && data.tiles_left != 0 && !data.dead {
                    x = x / 16;
                    y = y / 16;
                    if x < board[0].len() as i32 && y < board.len() as i32 && (board[y as usize][x as usize] >> 1) & 1 == 0 {
                        flag(x as usize, y as usize, board, &mut data.mines);
                    }
                }
            }
        },
        MouseButton::Middle => {
            if *state == ElementState::Pressed {
                let mut x: i32 = (data.mouse_x - 12.0) as i32;
                let mut y: i32 = (data.mouse_y - 55.0) as i32;
                if x >= 0 && y >= 0 && data.tiles_left != 0 && !data.dead {
                    x = x / 16;
                    y = y / 16;
                    if x < board[0].len() as i32 && y < board.len() as i32 {
                        if !data.placed_mines {
                            data.placed_mines = true;
                            data.start_time = SystemTime::now().duration_since(UNIX_EPOCH).expect("time went backwards lmao").as_secs();
                            place_mines(board, x as usize, y as usize, data.mines as u16, data);
                        }
                        click(x as usize, y as usize, board, data);
                        click_all_adj(board, x as usize, y as usize, data, click);
                    }
                }
            }
        },
        MouseButton::Other(_) => {}
    }
    true
}

#[inline]
pub fn mouse_moved(_device_id: &DeviceId, position: &PhysicalPosition<f64>, data: &mut Data) -> bool {
    data.mouse_x = position.x;
    data.mouse_y = position.y;
    true
}
