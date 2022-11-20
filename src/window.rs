use std::num::NonZeroU32;

use wgpu::*;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use winit::{event::*, event_loop::{ControlFlow, EventLoop}, window::WindowBuilder};
use winit::dpi::PhysicalSize;
use winit::window::Icon;
use winit::window::Window;

use crate::{assets, key_input, on_resize};
use crate::{render, mouse_click, Data, mouse_moved};
use crate::vertex_buffer_builder::VertexBufferBuilder;

pub async fn run() {
    let width = 9;
    let height = 9;
    let mine_count: u16 = 10;
    let event_loop = EventLoop::new();
    let flagged: Vec<u8> = assets::ICON.to_vec();

    let mut data: Data = Data::new(mine_count, width, height);
    let max_size = PhysicalSize::new(20 + 16 * 45, 20 + 16 * 45);
    let mut window = WindowBuilder::new().with_title("Minesweeper <3").with_window_icon(Some(Icon::from_rgba(flagged, 16, 16).unwrap())).with_resizable(true).with_min_inner_size(PhysicalSize::new(20 + 16 * 8, 63 + 16 * 3)).with_max_inner_size(max_size.clone()).with_inner_size(PhysicalSize::new((20 + 16 * width) as u32, (63 + 16 * height) as u32)).build(&event_loop).unwrap();
    let mut state = State::new(&window).await;

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                match state.render(&mut data) {
                    Ok(_) => {}
                    Err(SurfaceError::Lost) => state.resize(state.size),
                    Err(SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(_) => {}
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::WindowEvent { ref event, window_id } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        if physical_size.width <= max_size.width && physical_size.height <= max_size.height {
                            state.resize(*physical_size);
                        } else {
                            window.set_maximized(false);
                        }
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        state.resize(**new_inner_size);
                    }
                    _ => {}
                }
                state.input(&mut data, event, &mut window);
            },
            _ => {}
        }
    });
}

pub enum Theme {
    Dark,
    Light
}

pub struct State {
    surface: Surface,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
    size: PhysicalSize<u32>,
    render_pipeline: RenderPipeline,
    dark_diffuse_bind_group: BindGroup,
    light_diffuse_bind_group: BindGroup,
    pub theme: Theme
}

impl State {
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = Instance::new(Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .enumerate_adapters(Backends::all())
            .next()
            .unwrap();
        let (device, queue) = adapter.request_device(
            &DeviceDescriptor {
                features: Features::empty(),
                limits: if cfg!(target_arch = "wasm32") {
                    Limits::downlevel_webgl2_defaults()
                } else {
                    Limits::default()
                },
                label: None,
            },
            None,
        ).await.unwrap();
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: *surface.get_supported_formats(&adapter).get(0).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: PresentMode::Fifo, // fifo = vsync, immediate = no vsync, i want a framerate
            alpha_mode: CompositeAlphaMode::Auto
        };
        surface.configure(&device, &config);
        let width = 256;
        let height = 256;
        let texture_size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let dark_diffuse_texture = device.create_texture(
            &TextureDescriptor {
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                label: Some("dark_diffuse_texture"),
            }
        );
        queue.write_texture(
            ImageCopyTexture {
                texture: &dark_diffuse_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            assets::DARK_ATLAS,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(4 * width),
                rows_per_image: NonZeroU32::new(height),
            },
            texture_size,
        );
        let light_diffuse_texture = device.create_texture(
            &TextureDescriptor {
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                label: Some("light_diffuse_texture")
            }
        );
        queue.write_texture(
            ImageCopyTexture {
                texture: &light_diffuse_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            assets::LIGHT_ATLAS,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(4 * width),
                rows_per_image: NonZeroU32::new(height),
            },
            texture_size,
        );
        let dark_diffuse_texture_view = dark_diffuse_texture.create_view(&TextureViewDescriptor::default());
        let light_diffuse_texture_view = light_diffuse_texture.create_view(&TextureViewDescriptor::default());
        let diffuse_sampler = device.create_sampler(&SamplerDescriptor {
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            multisampled: false,
                            view_dimension: TextureViewDimension::D2,
                            sample_type: TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let dark_diffuse_bind_group = device.create_bind_group(
            &BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&dark_diffuse_texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&diffuse_sampler),
                    }
                ],
                label: Some("dark_diffuse_bind_group"),
            }
        );
        let light_diffuse_bind_group = device.create_bind_group(
            &BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&light_diffuse_texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&diffuse_sampler),
                    },
                ],
                label: Some("light_diffuse_bind_group"),
            }
        );
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shader"),
            source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });
        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "v",
                buffers: &[
                    VertexBufferLayout {
                        array_stride: 20,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &vertex_attr_array![0 => Float32x3, 1 => Float32x2]
                    }
                ]
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "f",
                targets: &[Some(ColorTargetState {
                    format: config.format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            dark_diffuse_bind_group,
            light_diffuse_bind_group,
            theme: {
                if rand::random::<u8>() > u8::MAX / 2 {
                    Theme::Dark
                } else {
                    Theme::Light
                }
            }
        }
    }

    #[inline]
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    #[inline]
    fn input(&mut self, data: &mut Data, event: &WindowEvent, window: &mut Window) {
        match event {
            WindowEvent::Resized(size) => on_resize(*size, data),
            WindowEvent::Moved(_) => (),
            WindowEvent::CloseRequested => (),
            WindowEvent::Destroyed => (),
            WindowEvent::DroppedFile(_) => (),
            WindowEvent::HoveredFile(_) => (),
            WindowEvent::HoveredFileCancelled => (),
            WindowEvent::ReceivedCharacter(_) => (),
            WindowEvent::Focused(_) => (),
            WindowEvent::KeyboardInput { input, .. } => key_input(*input, data, window, self),
            WindowEvent::ModifiersChanged(_) => (),
            WindowEvent::CursorMoved { position, .. } => mouse_moved(position, data, window, self),
            WindowEvent::CursorEntered { .. } => (),
            WindowEvent::CursorLeft { .. } => (),
            WindowEvent::MouseWheel { .. } => (),
            WindowEvent::MouseInput { state, button, .. } => mouse_click(state, button, data, window, self),
            WindowEvent::TouchpadPressure { .. } => (),
            WindowEvent::AxisMotion { .. } => (),
            WindowEvent::Touch(_) => (),
            WindowEvent::ScaleFactorChanged { .. } => (),
            WindowEvent::ThemeChanged(_) => (),
            WindowEvent::Ime(_) => (),
            WindowEvent::Occluded(_) => (),
        }

    }

    fn render(&mut self, data: &mut Data) -> Result<(), SurfaceError> {
        let vertex_buffer;
        let index_buffer;
        {
            let output: SurfaceTexture = self.surface.get_current_texture()?;
            let view: TextureView = output.texture.create_view(&TextureViewDescriptor::default());
            let mut encoder: CommandEncoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(match self.theme {
                            Theme::Dark => Color {
                                r: 0.0461488424,
                                g: 0.0461488424,
                                b: 0.0461488424,
                                a: 1.0
                            },
                            Theme::Light => Color {
                                r: 0.535641609,
                                g: 0.535641609,
                                b: 0.535641609,
                                a: 1.0
                            }
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            match self.theme {
                Theme::Dark => render_pass.set_bind_group(0, &self.dark_diffuse_bind_group, &[]),
                Theme::Light => render_pass.set_bind_group(0, &self.light_diffuse_bind_group, &[])
            }

            let mut vertex_buffer_builder = VertexBufferBuilder::new(&self.size, 256, 256);
            render(&mut vertex_buffer_builder, data);
            vertex_buffer = self.device.create_buffer_init(
                &BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: vertex_buffer_builder.vertices(),
                    usage: BufferUsages::VERTEX,
                }
            );
            index_buffer = self.device.create_buffer_init(
                &BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    contents: vertex_buffer_builder.indices(),
                    usage: BufferUsages::INDEX,
                }
            );
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint16);

            render_pass.draw_indexed(0..vertex_buffer_builder.indices_len(), 0, 0..1);

            drop(render_pass);

            self.queue.submit(std::iter::once(encoder.finish()));
            output.present();

        }
        Ok(())
    }
}