// ECS-powered 3D rendering with INSTANCED rendering
// Draws 1000s of entities in a single draw call
// See docs/research/ecs-choice.md for ECS architecture decisions

mod engine;

use winit::{
    event::{Event as WinitEvent, WindowEvent, ElementState, KeyEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};
use glam::{Mat4, Vec3};
use bevy_ecs::prelude::*;
use engine::{Transform, Velocity, Color as EntityColor};

// ============================================================================
// VERTEX DEFINITION
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

// ============================================================================
// INSTANCE DATA (per-entity)
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceData {
    position: [f32; 3],
    _padding: f32,  // Align to 16 bytes
    color: [f32; 4],
}

impl InstanceData {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,  // One per instance, not per vertex
            attributes: &[
                // Position (location 1)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Color (location 2)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

// Cube vertices (unit cube)
const CUBE_VERTICES: &[Vertex] = &[
    Vertex { position: [-0.1, -0.1,  0.1] },
    Vertex { position: [ 0.1, -0.1,  0.1] },
    Vertex { position: [ 0.1,  0.1,  0.1] },
    Vertex { position: [-0.1,  0.1,  0.1] },
    Vertex { position: [-0.1, -0.1, -0.1] },
    Vertex { position: [ 0.1, -0.1, -0.1] },
    Vertex { position: [ 0.1,  0.1, -0.1] },
    Vertex { position: [-0.1,  0.1, -0.1] },
];

const CUBE_INDICES: &[u16] = &[
    0, 1, 2,  0, 2, 3,  // Front
    5, 4, 7,  5, 7, 6,  // Back
    4, 0, 3,  4, 3, 7,  // Left
    1, 5, 6,  1, 6, 2,  // Right
    3, 2, 6,  3, 6, 7,  // Top
    4, 5, 1,  4, 1, 0,  // Bottom
];

// ============================================================================
// UNIFORM DATA (camera only)
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
}

impl Uniforms {
    fn new() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
}

// ============================================================================
// APPLICATION STATE
// ============================================================================

struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    num_indices: u32,
    max_instances: usize,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    // ECS World
    world: World,
    last_update: std::time::Instant,

    // Camera
    camera_distance: f32,
    camera_angle: f32,
}

impl State {
    async fn new(window: std::sync::Arc<winit::window::Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

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
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
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
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_instanced.wgsl").into()),
        });

        let uniforms = Uniforms::new();

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("uniform_bind_group_layout"),
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc(), InstanceData::desc()],  // Vertex + Instance buffers
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
            cache: None,
        });

        use wgpu::util::DeviceExt;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(CUBE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Create instance buffer (large enough for many instances)
        let max_instances = 10000;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (max_instances * std::mem::size_of::<InstanceData>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let num_indices = CUBE_INDICES.len() as u32;

        // Create ECS world and spawn test entities
        let mut world = World::new();
        spawn_test_entities(&mut world, 1000);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            num_indices,
            max_instances,
            uniform_buffer,
            uniform_bind_group,
            world,
            last_update: std::time::Instant::now(),
            camera_distance: 15.0,
            camera_angle: 0.0,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn update(&mut self) {
        let now = std::time::Instant::now();
        let dt = (now - self.last_update).as_secs_f32();
        self.last_update = now;

        self.camera_angle += 0.2 * dt;

        // Update ECS systems
        let bounds = Vec3::new(20.0, 10.0, 20.0);

        // Movement system
        let mut query = self.world.query::<(&mut Transform, &Velocity)>();
        for (mut transform, velocity) in query.iter_mut(&mut self.world) {
            transform.position += velocity.linear * dt;
        }

        // Bounds system
        let mut query = self.world.query::<&mut Transform>();
        let half_bounds = bounds / 2.0;
        for mut transform in query.iter_mut(&mut self.world) {
            if transform.position.x > half_bounds.x {
                transform.position.x = -half_bounds.x;
            } else if transform.position.x < -half_bounds.x {
                transform.position.x = half_bounds.x;
            }

            if transform.position.z > half_bounds.z {
                transform.position.z = -half_bounds.z;
            } else if transform.position.z < -half_bounds.z {
                transform.position.z = half_bounds.z;
            }

            if transform.position.y < 0.0 {
                transform.position.y = 0.0;
            } else if transform.position.y > bounds.y {
                transform.position.y = bounds.y;
            }
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Collect instance data from ECS BEFORE creating render pass
        let mut instance_data = Vec::new();
        let mut query = self.world.query::<(&Transform, &EntityColor)>();
        for (transform, color) in query.iter(&self.world) {
            instance_data.push(InstanceData {
                position: transform.position.to_array(),
                _padding: 0.0,
                color: [color.r, color.g, color.b, 1.0],
            });
        }

        let instance_count = instance_data.len().min(self.max_instances);

        // Write instance data to buffer BEFORE render pass
        if !instance_data.is_empty() {
            self.queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&instance_data[..instance_count]),
            );
        }

        // Update camera uniforms
        let aspect = self.size.width as f32 / self.size.height as f32;
        let projection = Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 100.0);

        let camera_x = self.camera_angle.cos() * self.camera_distance;
        let camera_z = self.camera_angle.sin() * self.camera_distance;
        let view_matrix = Mat4::look_at_rh(
            Vec3::new(camera_x, 8.0, camera_z),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::Y,
        );

        let view_proj = projection * view_matrix;
        let uniforms = Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // NOW create render pass (after all buffer writes)
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
                            r: 0.05,
                            g: 0.05,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));  // Instance data
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            // ONE DRAW CALL for all instances!
            render_pass.draw_indexed(0..self.num_indices, 0, 0..instance_count as u32);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

// ============================================================================
// ENTITY SPAWNING
// ============================================================================

fn spawn_test_entities(world: &mut World, count: usize) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..count {
        let position = Vec3::new(
            rng.gen_range(-10.0..10.0),
            rng.gen_range(0.0..8.0),
            rng.gen_range(-10.0..10.0),
        );

        let velocity = Vec3::new(
            rng.gen_range(-2.0..2.0),
            rng.gen_range(-1.0..1.0),
            rng.gen_range(-2.0..2.0),
        );

        world.spawn((
            Transform::from_position(position),
            Velocity::new(velocity),
            EntityColor::random(),
        ));
    }

    println!("Spawned {} entities", count);
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();

    let window_attributes = Window::default_attributes()
        .with_title("Flume Sugar - Instanced Rendering Demo (1000 entities, 1 draw call!)")
        .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));

    let window = std::sync::Arc::new(event_loop.create_window(window_attributes).unwrap());

    let mut state = pollster::block_on(State::new(window.clone()));
    let mut frame_count = 0;
    let mut last_fps_update = std::time::Instant::now();

    event_loop.run(move |event, control_flow| {
        match event {
            WinitEvent::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            state: ElementState::Pressed,
                            physical_key: PhysicalKey::Code(KeyCode::Escape),
                            ..
                        },
                    ..
                } => control_flow.exit(),
                WindowEvent::Resized(physical_size) => {
                    state.resize(*physical_size);
                }
                WindowEvent::RedrawRequested => {
                    state.update();
                    match state.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => control_flow.exit(),
                        Err(e) => eprintln!("{:?}", e),
                    }

                    frame_count += 1;
                    let now = std::time::Instant::now();
                    if (now - last_fps_update).as_secs_f32() >= 1.0 {
                        let entity_count = state.world.query::<&Transform>().iter(&state.world).count();
                        println!("FPS: {} | Entities: {} | Draw calls: 1", frame_count, entity_count);
                        frame_count = 0;
                        last_fps_update = now;
                    }
                }
                _ => {}
            },
            WinitEvent::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    }).unwrap();
}
