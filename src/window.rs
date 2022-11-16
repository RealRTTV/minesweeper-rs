use std::num::NonZeroU32;

use wgpu::*;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use winit::{event::*, event_loop::{ControlFlow, EventLoop}, window::WindowBuilder};
use winit::dpi::PhysicalSize;
use winit::window::Icon;
use winit::window::Window;

use crate::files::*;
use crate::{render, mouse_click, Data, mouse_moved};
use crate::vertex_buffer_builder::VertexBufferBuilder;

pub async fn run() {
    let width = 9;
    let height = 9;
    let mine_count: u16 = 10;
    let event_loop = EventLoop::new();
    let flagged: Vec<u8> = icon().to_vec();

    let mut data: Data = Data::new(mine_count, width, height);
    let window = WindowBuilder::new().with_title("Minesweeper").with_window_icon(Some(Icon::from_rgba(flagged, 16, 16).unwrap())).with_resizable(false).with_inner_size(PhysicalSize::new((20 + 16 * width) as u32, (63 + 16 * height) as u32)).build(&event_loop).unwrap();
    let mut state = State::new(&window).await;

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                match state.render(&mut data) {
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
                match event {
                    WindowEvent::CloseRequested { .. } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        state.resize(**new_inner_size);
                    }
                    event => state.input(&mut data, event)
                }
            },
            _ => {}
        }
    });
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    diffuse_bind_group: wgpu::BindGroup
}

impl State {
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .enumerate_adapters(wgpu::Backends::all())
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
            format: *surface.get_supported_formats(&adapter).get(0).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo, // fifo = vsync, immediate = no vsync, i want a framerate
            alpha_mode: CompositeAlphaMode::Auto
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
            ImageCopyTexture {
                texture: &diffuse_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
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
            &BufferInitDescriptor {
                label: Some("Temp Buffer"),
                contents: atlas,
                usage: BufferUsages::COPY_SRC,
            }
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("texture_buffer_copy_encoder"),
        });

        let bpr = NonZeroU32::new(4 * width);
        let rpi = NonZeroU32::new(height);
        encoder.copy_buffer_to_texture(
            ImageCopyBuffer {
                buffer: &buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: bpr,
                    rows_per_image: rpi,
                }
            },
            ImageCopyTexture {
                texture: &diffuse_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All
            },
            texture_size,
        );

        queue.submit(std::iter::once(encoder.finish()));

        let diffuse_texture_view = diffuse_texture.create_view(&TextureViewDescriptor::default());
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

        let diffuse_bind_group = device.create_bind_group(
            &BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&diffuse_texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&diffuse_sampler),
                    }
                ],
                label: Some("diffuse_bind_group"),
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
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
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

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            diffuse_bind_group
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
    fn input(&mut self, data: &mut Data, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(_) => (),
            WindowEvent::Moved(_) => (),
            WindowEvent::CloseRequested => (),
            WindowEvent::Destroyed => (),
            WindowEvent::DroppedFile(_) => (),
            WindowEvent::HoveredFile(_) => (),
            WindowEvent::HoveredFileCancelled => (),
            WindowEvent::ReceivedCharacter(_) => (),
            WindowEvent::Focused(_) => (),
            WindowEvent::KeyboardInput { .. } => (),
            WindowEvent::ModifiersChanged(_) => (),
            WindowEvent::CursorMoved { position, .. } => mouse_moved(position, data),
            WindowEvent::CursorEntered { .. } => (),
            WindowEvent::CursorLeft { .. } => (),
            WindowEvent::MouseWheel { .. } => (),
            WindowEvent::MouseInput { state, button, .. } => mouse_click(state, button, data),
            WindowEvent::TouchpadPressure { .. } => (),
            WindowEvent::AxisMotion { .. } => (),
            WindowEvent::Touch(_) => (),
            WindowEvent::ScaleFactorChanged { .. } => (),
            WindowEvent::ThemeChanged(_) => (),
            WindowEvent::Ime(_) => (),
            WindowEvent::Occluded(_) => (),
        }

    }

    fn render(&mut self, data: &mut Data) -> Result<(), wgpu::SurfaceError> {
        let vertex_buffer;
        let index_buffer;
        {
            let output: SurfaceTexture = self.surface.get_current_texture()?;
            let view: TextureView = output.texture.create_view(&TextureViewDescriptor::default());
            let mut encoder: CommandEncoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color {
                            r: 0.392,
                            g: 0.584,
                            b: 0.929,
                            a: 1.0
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);

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