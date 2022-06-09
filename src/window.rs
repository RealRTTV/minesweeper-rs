use std::num::NonZeroU32;
use std::ptr::null;

use wgpu::{CommandEncoder, ImageDataLayout, SurfaceTexture, TextureAspect, TextureView};
use winit::platform::windows::WindowBuilderExtWindows;
use wgpu::util::DeviceExt;
use winapi::shared::minwindef::UINT;
use winapi::um::winnt::LPCWSTR;
use winapi::um::winuser::{AppendMenuW, CreateMenu, MF_POPUP, MF_SEPARATOR, MF_STRING};
use winit::{event::*, event_loop::{ControlFlow, EventLoop}, window::WindowBuilder};
use winit::dpi::PhysicalSize;
use winit::window::Icon;
use winit::window::Window;
use wchar::wchz;

use crate::files::*;
use crate::{render, mouse_click, Data, mouse_moved};

pub async fn run() {
    let width = 99;
    let height = 99;
    let mine_count: u16 = 99;
    env_logger::init();
    let event_loop = EventLoop::new();
    let flagged: Vec<u8> = icon().to_vec();

    let mut board: Vec<Vec<u8>> = vec!(vec!(0u8; width); height);
    let mut data: Data = Data::new((board.len() * board[0].len()) as u16 - mine_count, mine_count);
    let menubar = unsafe { CreateMenu() };
    let game = unsafe { CreateMenu() };
    let help = unsafe { CreateMenu() };
    unsafe {
        AppendMenuW(game, MF_STRING, 1, wchz!("New") as LPCWSTR); // f2 keybind thing - n
        AppendMenuW(game, MF_SEPARATOR, 0, null());
        AppendMenuW(game, MF_STRING, 2, wchz!("Beginner") as LPCWSTR); // checkbox - b
        AppendMenuW(game, MF_STRING, 3, wchz!("Intermediate") as LPCWSTR); // checkbox - i
        AppendMenuW(game, MF_STRING, 4, wchz!("Expert") as LPCWSTR); // checkbox - e
        AppendMenuW(game, MF_STRING, 5, wchz!("Custom...") as LPCWSTR); // checkbox - c
        AppendMenuW(game, MF_SEPARATOR, 0, null());
        AppendMenuW(game, MF_STRING, 6, wchz!("Marks (?)") as LPCWSTR); // checkbox - m
        AppendMenuW(game, MF_STRING, 7, wchz!("Color") as LPCWSTR); // checkbox - l
        AppendMenuW(game, MF_STRING, 8, wchz!("Sound") as LPCWSTR); // checkbox - s
        AppendMenuW(game, MF_SEPARATOR, 0, null());
        AppendMenuW(game, MF_STRING, 8, wchz!("Best Times...") as LPCWSTR); // clickable - t
        AppendMenuW(game, MF_SEPARATOR, 0, null());
        AppendMenuW(game, MF_STRING, 9, wchz!("Exit") as LPCWSTR); // x
        AppendMenuW(menubar, MF_POPUP, (game as UINT).try_into().unwrap(), wchz!("Game") as LPCWSTR); // g

        AppendMenuW(help, MF_STRING, 10, wchz!("Contents") as LPCWSTR); // c
        AppendMenuW(help, MF_STRING, 11, wchz!("Search for Help on") as LPCWSTR); // h
        AppendMenuW(help, MF_STRING, 12, wchz!("Using Help") as LPCWSTR); // s
        AppendMenuW(help, MF_SEPARATOR, 0, null());
        AppendMenuW(help, MF_STRING, 13, wchz!("About Minesweeper...") as LPCWSTR); // a
        AppendMenuW(menubar, MF_POPUP, (help as UINT).try_into().unwrap(), wchz!("Help") as LPCWSTR);
    };
    let window = WindowBuilder::new().with_title("Minesweeper").with_window_icon(Some(Icon::from_rgba(flagged, 16, 16).unwrap())).with_resizable(false).with_inner_size(PhysicalSize::new((20 + 16 * board[0].len()) as i32, (63 + 16 * board.len()) as i32)).with_menu(menubar).build(&event_loop).unwrap();
    let mut state = State::new(&window).await;

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                state.update();
                match state.render(&mut board, &data) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(_) => {}
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::WindowEvent {
                ref event,
                window_id,
            }

            if window_id == window.id() => {
                state.redraw = state.input(&mut data, event, &mut board);
                if !(state.redraw) {
                    match event {
                        WindowEvent::CloseRequested { .. } => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            },
            _ => {}
        }
    });
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

const VERTICES: &[Vertex] = &[
    Vertex { position: [1.0, 1.0, 0.0], tex_coords: [1.0, 0.0] },
    Vertex { position: [-1.0, 1.0, 0.0], tex_coords: [0.0, 0.0] },
    Vertex { position: [-1.0, -1.0, 0.0], tex_coords: [0.0, 1.0] },
    Vertex { position: [1.0, -1.0, 0.0], tex_coords: [1.0, 1.0] },
];

const INDICES: &[u16] = &[
    0, 1, 2,
    0, 2, 3,
];

impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ]
        }
    }

    pub fn new(x: f32, y: f32, z: f32, u: f32, v: f32) -> Vertex {
        Vertex { position: [x, y, z], tex_coords: [u, v] }
    }
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    diffuse_bind_group: wgpu::BindGroup,
    redraw: bool
}

impl State {
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .enumerate_adapters(wgpu::Backends::all())
            .filter(|adapter| {
                surface.get_preferred_format(&adapter).is_some()
            })
            .next()
            .unwrap();
        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                label: None,
            },
            None,
        ).await.unwrap();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate, // fifo = vsync, immediate = no vsync
        };
        surface.configure(&device, &config);
        let width = 256;
        let height = 256;
        let atlas = atlas();
        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let diffuse_texture = device.create_texture(
            &wgpu::TextureDescriptor {
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                label: Some("diffuse_texture"),
            }
        );
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &diffuse_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            atlas,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(4 * width),
                rows_per_image: NonZeroU32::new(height),
            },
            texture_size,
        );
        let buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Temp Buffer"),
                contents: &atlas,
                usage: wgpu::BufferUsages::COPY_SRC,
            }
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("texture_buffer_copy_encoder"),
        });

        let bpr = NonZeroU32::try_from(4 * width);
        let rpi = NonZeroU32::try_from(height);
        encoder.copy_buffer_to_texture(
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: bpr.ok(),
                    rows_per_image: rpi.ok(),
                }
            },
            wgpu::ImageCopyTexture {
                texture: &diffuse_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: TextureAspect::All
            },
            texture_size,
        );

        queue.submit(std::iter::once(encoder.finish()));

        let diffuse_texture_view = diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let diffuse_bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                    }
                ],
                label: Some("diffuse_bind_group"),
            }
        );

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    Vertex::desc(),
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });
        let vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );
        let index_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(INDICES),
                usage: wgpu::BufferUsages::INDEX,
            }
        );

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            diffuse_bind_group,
            redraw: true
        }
    }

    #[inline]
    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    #[inline]
    fn input(&mut self, data: &mut Data, event: &WindowEvent, board: &mut Vec<Vec<u8>>) -> bool {
        match event {
            WindowEvent::Resized(_) => false,
            WindowEvent::Moved(_) => false,
            WindowEvent::CloseRequested => false,
            WindowEvent::Destroyed => false,
            WindowEvent::DroppedFile(_) => false,
            WindowEvent::HoveredFile(_) => false,
            WindowEvent::HoveredFileCancelled => false,
            WindowEvent::ReceivedCharacter(_) => false,
            WindowEvent::Focused(_) => false,
            WindowEvent::KeyboardInput { .. } => false,
            WindowEvent::ModifiersChanged(_) => false,
            WindowEvent::CursorMoved { device_id, position, .. } => mouse_moved(device_id, position, data),
            WindowEvent::CursorEntered { .. } => false,
            WindowEvent::CursorLeft { .. } => false,
            WindowEvent::MouseWheel { .. } => false,
            WindowEvent::MouseInput { device_id, state, button, .. } => mouse_click(device_id, state, button, data, board),
            WindowEvent::TouchpadPressure { .. } => false,
            WindowEvent::AxisMotion { .. } => false,
            WindowEvent::Touch(_) => false,
            WindowEvent::ScaleFactorChanged { .. } => false,
            WindowEvent::ThemeChanged(_) => false
        }

    }

    fn update(&mut self) {

    }

    fn render(&mut self, mut board: &mut Vec<Vec<u8>>, data: &Data) -> Result<(), wgpu::SurfaceError> {
        if !self.redraw {
            return Ok(());
        }

        println!("Redrawing!");
        self.redraw = false;

        let output: SurfaceTexture = self.surface.get_current_texture()?;
        let view: TextureView = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder: CommandEncoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.392,
                        g: 0.584,
                        b: 0.929,
                        a: 1.0
                    }),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        render(&mut vertices, &mut indices, &self.size, &mut board, data);
        self.vertex_buffer = self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(vertices.as_slice()),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );
        self.index_buffer = self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(indices.as_slice()),
                usage: wgpu::BufferUsages::INDEX,
            }
        );
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);

        drop(render_pass);

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}