use winit::dpi::PhysicalSize;
use crate::window::Vertex;

#[inline]
pub fn draw_texture(vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>, size: &PhysicalSize<u32>, x: u32, y: u32, u: u32, v: u32, width: u32, height: u32) {
    draw_texture_with_z(vertices, indices, size, x, y, 0.0, u, v, width, height);
}

pub fn draw_texture_with_z(vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>, size: &PhysicalSize<u32>, x: u32, y: u32, z: f32, u: u32, v: u32, width: u32, height: u32) {
    let x0 = ((x as f32 / size.width as f32) as f32 * 2.0) - 1.0; // todo, fix math
    let x1 = (((x + width) as f32 / size.width as f32) as f32 * 2.0) - 1.0;
    let y0 = -((((y + height) as f32 / size.height as f32) as f32 * 2.0) - 1.0);
    let y1 = -(((y as f32 / size.height as f32) as f32 * 2.0) - 1.0);
    let u0 = u as f32 / 256.0;
    let u1 = (u + width) as f32 / 256.0;
    let v0 = v as f32 / 256.0;
    let v1 = (v + height) as f32 / 256.0;
    vertices.push(Vertex::new(x1, y1, z, u1, v0)); // top right
    vertices.push(Vertex::new(x0, y1, z, u0, v0)); // top left
    vertices.push(Vertex::new(x0, y0, z, u0, v1)); // bottom left
    vertices.push(Vertex::new(x1, y0, z, u1, v1)); // bottom right
    let option = indices.iter().reduce(|x, y| {if x > y {x} else {y}});
    let len = if option.is_some() {option.unwrap() + 1} else {0};
    indices.push((len + 0) as u16);
    indices.push((len + 1) as u16);
    indices.push((len + 2) as u16);
    indices.push((len + 0) as u16);
    indices.push((len + 2) as u16);
    indices.push((len + 3) as u16);
}
