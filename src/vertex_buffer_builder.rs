use winit::dpi::PhysicalSize;

pub struct VertexBufferBuilder {
    vertices: Vec<u8>,
    indices: Vec<u8>,
    vertices_len: u32,
    window_width: f32,
    window_height: f32,
    texture_width: f32,
    texture_height: f32
}

impl VertexBufferBuilder {
    pub fn new(size: &PhysicalSize<u32>, texture_width: u32, texture_height: u32) -> VertexBufferBuilder {
        VertexBufferBuilder {
            vertices: Vec::with_capacity(393216),
            indices: Vec::with_capacity(131072),
            vertices_len: 0,
            window_width: size.width as f32,
            window_height: size.height as f32,
            texture_width: texture_width as f32,
            texture_height: texture_height as f32
        }
    }

    #[inline]
    pub fn window_height(&self) -> u32 {
        self.window_height as u32
    }

    #[inline]
    pub fn window_width(&self) -> u32 {
        self.window_width as u32
    }

    #[inline]
    pub fn vertices(&self) -> &[u8] {
        &self.vertices
    }

    #[inline]
    pub fn indices(&self) -> &[u8] {
        &self.indices
    }
    #[inline]
    pub fn indices_len(&self) -> u32 {
        (self.indices.len() >> 1) as u32
    }

    #[inline]
    pub fn draw_texture(&mut self, pos: (u32, u32), uv: (u32, u32), dims: (u32, u32)) {
        self.draw_texture_z(pos, 0.0, uv, dims);
    }

    #[inline]
    pub fn draw_texture_z(&mut self, pos: (u32, u32), z: f32, uv: (u32, u32), dims: (u32, u32)) {
        unsafe {
            let x = pos.0 as f32;
            let y = pos.1 as f32;
            let u = uv.0 as f32;
            let v = uv.1 as f32;
            let width = dims.0 as f32;
            let height = dims.1 as f32;

            let x0 = (x / self.window_width) * 2.0f32 - 1.0f32;
            let x1 = x0 + (2.0 * width) / self.window_width;
            let y1 = (y / self.window_height) * -2.0 + 1.0;
            let y0 = y1 + (-2.0 * height) / self.window_height;
            let u0 = u / self.texture_width;
            let u1 = (u + width) / self.texture_width;
            let v0 = v / self.texture_height;
            let v1 = (v + height) / self.texture_height;
            let z = z;

            let len = self.vertices_len;
            let vec = &mut self.vertices;

            let vertices_len = vec.len();
            let ptr = vec.as_mut_ptr().add(vertices_len) as *mut f32;
            // top left
            *ptr                = x1;
            *(ptr.add(1)) = y1;
            *(ptr.add(2)) = z;
            *(ptr.add(3)) = u1;
            *(ptr.add(4)) = v0;
            // top right
            *(ptr.add(5)) = x0;
            *(ptr.add(6)) = y1;
            *(ptr.add(7)) = z;
            *(ptr.add(8)) = u0;
            *(ptr.add(9)) = v0;
            // bottom left
            *(ptr.add(10)) = x0;
            *(ptr.add(11)) = y0;
            *(ptr.add(12)) = z;
            *(ptr.add(13)) = u0;
            *(ptr.add(14)) = v1;
            // bottom right
            *(ptr.add(15)) = x1;
            *(ptr.add(16)) = y0;
            *(ptr.add(17)) = z;
            *(ptr.add(18)) = u1;
            *(ptr.add(19)) = v1;

            vec.set_len(vertices_len + 80);

            let indices_len = self.indices.len();
            let ptr = self.indices.as_mut_ptr().add(indices_len);

            *ptr                 =   len            as u8;
            *(ptr.add(1))  =  (len >> 8)      as u8;
            *(ptr.add(2))  =  (len + 1)       as u8;
            *(ptr.add(3))  = ((len + 1) >> 8) as u8;
            *(ptr.add(4))  =  (len + 2)       as u8;
            *(ptr.add(5))  = ((len + 2) >> 8) as u8;
            *(ptr.add(6))  = *ptr;
            *(ptr.add(7))  = *(ptr.add(1));
            *(ptr.add(8))  = *(ptr.add(4));
            *(ptr.add(9))  = *(ptr.add(5));
            *(ptr.add(10)) =  (len + 3)       as u8;
            *(ptr.add(11)) = ((len + 3) >> 8) as u8;

            self.indices.set_len(indices_len + 12);

            self.vertices_len += 4;
        }
    }
}
