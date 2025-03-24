use std::iter;
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};
use rand::prelude::*;
use glam::Vec2;

// Number of particles to simulate
const NUM_PARTICLES: usize = 5000;
// Size of particles in pixels
const PARTICLE_SIZE: f32 = 3.0;
// Gravity force
const GRAVITY: Vec2 = Vec2::new(0.0, 9.8);
// Damping factor for collisions (0.0 = no bounce, 1.0 = perfect bounce)
const BOUNCE_DAMPING: f32 = 0.7;
// Random initial velocity range
const INITIAL_VELOCITY_RANGE: f32 = 50.0;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ParticleInstance {
    position: [f32; 2],
    color: [f32; 4],
}

impl ParticleInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

struct Particle {
    position: Vec2,
    velocity: Vec2,
    color: [f32; 4],
}

impl Particle {
    fn new(position: Vec2, velocity: Vec2, color: [f32; 4]) -> Self {
        Self {
            position,
            velocity,
            color,
        }
    }

    fn update(&mut self, dt: f32, width: f32, height: f32) {
        // Apply gravity
        self.velocity += GRAVITY * dt;

        // Update position
        self.position += self.velocity * dt;

        // Boundary collision detection and response
        let radius = PARTICLE_SIZE / 2.0;

        // Bottom boundary
        if self.position.y + radius > height {
            self.position.y = height - radius;
            self.velocity.y = -self.velocity.y * BOUNCE_DAMPING;
        }

        // Top boundary
        if self.position.y - radius < 0.0 {
            self.position.y = radius;
            self.velocity.y = -self.velocity.y * BOUNCE_DAMPING;
        }

        // Right boundary
        if self.position.x + radius > width {
            self.position.x = width - radius;
            self.velocity.x = -self.velocity.x * BOUNCE_DAMPING;
        }

        // Left boundary
        if self.position.x - radius < 0.0 {
            self.position.x = radius;
            self.velocity.x = -self.velocity.x * BOUNCE_DAMPING;
        }
    }

    fn to_instance(&self) -> ParticleInstance {
        ParticleInstance {
            position: [self.position.x, self.position.y],
            color: self.color,
        }
    }
}

struct ParticleSimulation {
    particles: Vec<Particle>,
}

impl ParticleSimulation {
    fn new(width: f32, height: f32) -> Self {
        let mut rng = rand::thread_rng();
        let mut particles = Vec::with_capacity(NUM_PARTICLES);

        for _ in 0..NUM_PARTICLES {
            let x = rng.gen_range(0.0..width);
            let y = rng.gen_range(0.0..height);
            let vx = rng.gen_range(-INITIAL_VELOCITY_RANGE..INITIAL_VELOCITY_RANGE);
            let vy = rng.gen_range(-INITIAL_VELOCITY_RANGE..INITIAL_VELOCITY_RANGE);

            // Create a random color with full alpha
            let r = rng.gen_range(0.3..1.0);
            let g = rng.gen_range(0.3..1.0);
            let b = rng.gen_range(0.3..1.0);

            particles.push(Particle::new(
                Vec2::new(x, y),
                Vec2::new(vx, vy),
                [r, g, b, 1.0],
            ));
        }

        Self { particles }
    }

    fn update(&mut self, dt: f32, width: f32, height: f32) {
        for particle in &mut self.particles {
            particle.update(dt, width, height);
        }
    }

    fn get_instance_data(&self) -> Vec<ParticleInstance> {
        self.particles.iter().map(|p| p.to_instance()).collect()
    }
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    simulation: ParticleSimulation,
    instance_buffer: wgpu::Buffer,
}

impl State {
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        // Initialize the particle simulation
        let simulation = ParticleSimulation::new(size.width as f32, size.height as f32);
        let instance_data = simulation.get_instance_data();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        // Define the vertices of a square to be instanced for each particle
        let vertex_data = [
            Vertex { position: [-0.5, -0.5] },
            Vertex { position: [0.5, -0.5] },
            Vertex { position: [0.5, 0.5] },
            Vertex { position: [-0.5, 0.5] },
            Vertex { position: [-0.5, -0.5] },
            Vertex { position: [0.5, 0.5] },
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertex_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create the shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc(), ParticleInstance::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
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

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            simulation,
            instance_buffer,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            
            // Recreate the shader with new dimensions
            let shader_code = include_str!("shader_code.wgsl")
                .replace("PARTICLE_SIZE: f32 = 3.0", &format!("PARTICLE_SIZE: f32 = {}", PARTICLE_SIZE))
                .replace("SCREEN_WIDTH: f32 = 800.0", &format!("SCREEN_WIDTH: f32 = {}", new_size.width as f32))
                .replace("SCREEN_HEIGHT: f32 = 600.0", &format!("SCREEN_HEIGHT: f32 = {}", new_size.height as f32));
                
            let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_code.into()),
            });
            
            // Recreate the render pipeline with the new shader
            let render_pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });
            
            self.render_pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc(), ParticleInstance::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.config.format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::REPLACE,
                            alpha: wgpu::BlendComponent::REPLACE,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
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
        }
    }

    fn update(&mut self, dt: f32) {
        self.simulation.update(dt, self.size.width as f32, self.size.height as f32);

        // Update instance buffer with new particle positions
        let instance_data = self.simulation.get_instance_data();
        self.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&instance_data),
        );
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.draw(0..6, 0..NUM_PARTICLES as u32);
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Rust Particle Simulation")
        .with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
        .build(&event_loop)
        .unwrap();

    let mut state = pollster::block_on(State::new(&window));
    let mut last_update_instant = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        match event {
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
                WindowEvent::Resized(physical_size) => {
                    state.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    state.resize(**new_inner_size);
                }
                _ => {}
            },
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                let now = Instant::now();
                let dt = now.duration_since(last_update_instant).as_secs_f32();
                last_update_instant = now;

                state.update(dt);
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}
