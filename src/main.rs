use bytemuck::{Pod, Zeroable};
use pollster::FutureExt as _;
use std::{mem, time::Instant};
use tracing::debug;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

const MAX_SPHERES_COUNT: usize = 100;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct RayTracingParams {
    camera_pos: [f32; 4],
    aspect_ratio: f32,
    spheres_count: u32,
    time_elapsed: f32,
    _padding: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Sphere {
    pos: [f32; 3],
    r: f32,
    vel: [f32; 4],
}

const SAMPLE_COUNT: u32 = 4;

struct Renderer {
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    queue: wgpu::Queue,
    device: wgpu::Device,
    render_pipeline: wgpu::RenderPipeline,
    compute_pipeline: wgpu::ComputePipeline,
    uniform_buffer: wgpu::Buffer,
    render_bind_groups: [wgpu::BindGroup; 2],
    compute_bind_groups: [wgpu::BindGroup; 2],
    multisampled_framebuffer: wgpu::TextureView,
    camera_x: f32,
    aspect_ratio: f32,
    spheres_count: u32,
    frame_start: Instant,
}

fn init_spheres() -> Vec<Sphere> {
    return vec![
        Sphere {
            pos: [0.0, 0.0, 6.0],
            r: 1.0,
            vel: [1.0, 0.0, 0.0, 0.0],
        },
        Sphere {
            pos: [2.0, 1.0, 8.0],
            r: 0.5,
            vel: [0.0, 1.0, 0.0, 0.0],
        },
        Sphere {
            pos: [-2.0, -1.0, 8.0],
            r: 0.3,
            vel: [0.0, 0.0, 0.1, 0.0],
        },
    ];
}

impl Renderer {
    async fn new(window: &Window) -> Self {
        let instance = wgpu::Instance::default();
        let size = window.inner_size();
        let surface = unsafe { instance.create_surface(window) }.expect("Can't create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("Can't get an adapter.");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY,
                    limits: adapter.limits(),
                },
                None,
            )
            .await
            .expect("Can't get a device.");
        //Buffers
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: mem::size_of::<RayTracingParams>() as _,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let spheres = init_spheres();
        let spheres_buffers = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (mem::size_of::<Sphere>() * MAX_SPHERES_COUNT) as _,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (mem::size_of::<Sphere>() * MAX_SPHERES_COUNT) as _,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        ];
        for i in 0..2 {
            queue.write_buffer(&spheres_buffers[i], 0, bytemuck::cast_slice(&spheres));
        }
        // Compute pipeline
        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "compute.wgsl"
            ))),
        });
        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                mem::size_of::<RayTracingParams>() as _,
                            ),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                (mem::size_of::<Sphere>() * MAX_SPHERES_COUNT) as _,
                            ),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                (mem::size_of::<Sphere>() * MAX_SPHERES_COUNT) as _,
                            ),
                        },
                        count: None,
                    },
                ],
            });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: "main",
        });
        let compute_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: spheres_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: spheres_buffers[1].as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: spheres_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: spheres_buffers[0].as_entire_binding(),
                    },
                ],
            }),
        ];
        // Graphics pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shader.wgsl"
            ))),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            mem::size_of::<RayTracingParams>() as _
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            (mem::size_of::<Sphere>() * MAX_SPHERES_COUNT) as _,
                        ),
                    },
                    count: None,
                },
            ],
        });
        let render_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: spheres_buffers[0].as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: spheres_buffers[1].as_entire_binding(),
                    },
                ],
            }),
        ];
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(swapchain_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 4,
                ..Default::default()
            },
            multiview: None,
        });

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        let multisampled_texture_extend = wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };
        let multisampled_texture_descriptor = wgpu::TextureDescriptor {
            label: None,
            size: multisampled_texture_extend,
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: swapchain_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        let multisampled_framebuffer = device
            .create_texture(&multisampled_texture_descriptor)
            .create_view(&wgpu::TextureViewDescriptor::default());

        Renderer {
            surface,
            surface_config,
            queue,
            device,
            render_pipeline,
            compute_pipeline,
            uniform_buffer,
            render_bind_groups,
            compute_bind_groups,
            multisampled_framebuffer,
            camera_x: 0.0f32,
            aspect_ratio: (size.width as f32) / (size.height as f32),
            spheres_count: spheres.len() as u32,
            frame_start: Instant::now(),
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.aspect_ratio = (width as f32) / (height as f32);
        self.surface.configure(&self.device, &self.surface_config);
        let multisampled_texture_extend = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let multisampled_texture_descriptor = wgpu::TextureDescriptor {
            label: None,
            size: multisampled_texture_extend,
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        self.multisampled_framebuffer = self
            .device
            .create_texture(&multisampled_texture_descriptor)
            .create_view(&wgpu::TextureViewDescriptor::default());
    }

    fn render(&mut self, odd_frame: bool) {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Can't get swap chain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[RayTracingParams {
                camera_pos: [self.camera_x, 0.0, -1.0, 0.0],
                aspect_ratio: self.aspect_ratio,
                spheres_count: self.spheres_count as u32,
                time_elapsed: self.frame_start.elapsed().as_secs_f32(),
                _padding: 0.0,
            }]),
        );
        {
            let mut cpass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
            cpass.set_pipeline(&self.compute_pipeline);
            cpass.set_bind_group(
                0,
                &(self.compute_bind_groups[if odd_frame { 1 } else { 0 }]),
                &[],
            );
            cpass.dispatch_workgroups(self.spheres_count, 1, 1);
        }
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.multisampled_framebuffer,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: false,
                    },
                })],
                depth_stencil_attachment: None,
            });
            rpass.set_pipeline(&self.render_pipeline);
            rpass.set_bind_group(
                0,
                &(self.render_bind_groups[if odd_frame { 1 } else { 0 }]),
                &[],
            );
            rpass.draw(0..6, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        self.frame_start = Instant::now();
    }

    fn move_x(&mut self, delta: f32) {
        self.camera_x += delta;
    }
}

async fn run() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .build(&event_loop)
        .expect("Can't create window");
    let mut frames_start = Instant::now();
    let mut frame_count = 0;
    let mut renderer = Renderer::new(&window).await;
    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            WindowEvent::Resized(size) => {
                renderer.resize(size.width, size.height);
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Left),
                        ..
                    },
                ..
            } => {
                renderer.move_x(0.1);
                renderer.render(frame_count % 2 == 1);
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Right),
                        ..
                    },
                ..
            } => {
                renderer.move_x(-0.1);
                renderer.render(frame_count % 2 == 1);
            }
            _ => {}
        },
        Event::RedrawRequested(_) => {
            frame_count += 1;
            if frame_count >= 1000 {
                let elapsed_time = frames_start.elapsed().as_secs_f32();
                println!(
                    "Avg frame time {}ms, {} fps",
                    elapsed_time * 1000.0 / frame_count as f32,
                    frame_count as f32 / elapsed_time
                );
                frame_count = 0;
                frames_start = Instant::now();
            }
            renderer.render(frame_count % 2 == 1);
        }
        Event::RedrawEventsCleared => {
            window.request_redraw();
        }
        _ => {}
    });
}

fn main() {
    tracing_subscriber::fmt::init();
    debug!("Starting");
    run().block_on();
}
