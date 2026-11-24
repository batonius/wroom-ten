use bytemuck::{Pod, Zeroable};
use pollster::FutureExt as _;
use rand::{distr::Uniform, rng, RngExt};
use std::{mem, sync::Arc, time::Instant};
use tracing::debug;
use winit::{
    event_loop::{ControlFlow, EventLoop, OwnedDisplayHandle},
    window::Window,
};

const MAX_SPHERES_COUNT: usize = 1000;

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
    color: [f32; 4],
}

const SAMPLE_COUNT: u32 = 4;

struct Renderer {
    surface: wgpu::Surface<'static>,
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
    window: Arc<Window>,
}

fn init_spheres() -> Vec<Sphere> {
    let mut spheres = Vec::with_capacity(MAX_SPHERES_COUNT);
    let mut rng = rng();
    let x_dist = Uniform::new(-3.0, 3.0).unwrap();
    let y_dist = Uniform::new(-1.0, 1.0).unwrap();
    let z_dist = Uniform::new(1.0, 15.0).unwrap();
    let r_dist = Uniform::new(0.05, 0.1).unwrap();
    let vel_dist = Uniform::new(0.0, 0.5).unwrap();
    let rgb_dist = Uniform::new(0.0, 0.5).unwrap();
    let refl_dist = Uniform::new(0.0, 0.9).unwrap();
    for _ in 0..MAX_SPHERES_COUNT {
        spheres.push(Sphere {
            pos: [rng.sample(x_dist), rng.sample(y_dist), rng.sample(z_dist)],
            r: rng.sample(r_dist),
            vel: [
                rng.sample(vel_dist),
                rng.sample(vel_dist),
                rng.sample(vel_dist),
                0.0,
            ],
            color: [
                rng.sample(rgb_dist),
                rng.sample(rgb_dist),
                rng.sample(rgb_dist),
                rng.sample(refl_dist),
            ],
        })
    }
    spheres
}

impl Renderer {
    async fn new(display: OwnedDisplayHandle, window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(display),
        ));
        let size = window.inner_size();
        let surface = instance
            .create_surface(window.clone())
            .expect("Can't create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                apply_limit_buckets: false,
            })
            .await
            .expect("Can't get an adapter.");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY,
                required_limits: adapter.limits(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
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
        for sphere_buffer in &spheres_buffers {
            queue.write_buffer(sphere_buffer, 0, bytemuck::cast_slice(&spheres));
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
                bind_group_layouts: &[Some(&compute_bind_group_layout)],
                immediate_size: 0,
            });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
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
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(swapchain_format.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 4,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
            color_space: Default::default(),
            desired_maximum_frame_latency: 0,
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
            window,
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
        let wgpu::CurrentSurfaceTexture::Success(frame) = self.surface.get_current_texture() else {
            return;
        };
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
                spheres_count: self.spheres_count,
                time_elapsed: self.frame_start.elapsed().as_secs_f32(),
                _padding: 0.0,
            }]),
        );
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
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
                        store: wgpu::StoreOp::Discard,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
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
        self.queue.present(frame);
        self.frame_start = Instant::now();
    }

    fn request_redraw(&self) {
        self.window.request_redraw()
    }
}

struct App {
    renderer: Option<Renderer>,
    frame_count: u64,
    frames_start: Instant,
}

impl App {
    fn new() -> Self {
        App {
            renderer: None,
            frame_count: 0,
            frames_start: Instant::now(),
        }
    }
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
        let renderer = pollster::block_on(Renderer::new(
            event_loop.owned_display_handle(),
            window.clone(),
        ));
        self.renderer = Some(renderer);
        window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let renderer = self.renderer.as_mut().unwrap();
        match event {
            winit::event::WindowEvent::CloseRequested => event_loop.exit(),
            winit::event::WindowEvent::RedrawRequested => {
                self.frame_count += 1;
                if self.frame_count >= 1000 {
                    let elapsed_time = self.frames_start.elapsed().as_secs_f32();
                    println!(
                        "Avg frame time {}ms, {} fps",
                        elapsed_time * 1000.0 / self.frame_count as f32,
                        self.frame_count as f32 / elapsed_time
                    );
                    self.frame_count = 0;
                    self.frames_start = Instant::now();
                }
                renderer.render(self.frame_count % 2 == 1);
                renderer.request_redraw();
            }
            winit::event::WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
            _ => (),
        }
    }
}

async fn run() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}

fn main() {
    tracing_subscriber::fmt::init();
    debug!("Starting");
    run().block_on();
}
