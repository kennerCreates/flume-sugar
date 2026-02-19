// Procedural modeling demo: single vertex → Skin Modifier → Catmull-Clark ×2 → near-sphere
// See docs/research/procedural-modeling.md for pipeline decisions.

mod engine;

use winit::{
    application::ApplicationHandler,
    event::{WindowEvent, ElementState, KeyEvent},
    event_loop::{EventLoop, ActiveEventLoop, ControlFlow},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};
use glam::{Mat4, Vec3};
use bevy_ecs::prelude::*;
use engine::{Transform, Color as EntityColor, Velocity, GroupMembership, UnitAgent};
use engine::{NavigationGrid, FlowField, compute_flowfield, GRID_WIDTH, GRID_HEIGHT};
use engine::{AgentSnapshot, SpatialGrid, compute_orca_velocity};
use engine::camera::RtsCamera;
use engine::debug_overlay::{DebugOverlay, DebugStats, UnitDebugDraw};
use engine::input::InputState;
use engine::mesh::GpuVertex;
use egui;

/// Movement speed for all units (world units per second).
const UNIT_SPEED: f32 = 2.5;
/// Physical collision radius of each unit (matches the procedural sphere mesh).
const UNIT_RADIUS: f32 = 0.5;
/// Units within this world-space distance of their goal are considered arrived.
const ARRIVAL_RADIUS: f32 = 1.5;
/// ORCA look-ahead window (seconds).  1.5 s gives smooth anticipatory avoidance.
const ORCA_TIME_HORIZON: f32 = 1.5;

// ============================================================================
// INSTANCE DATA (per-entity, passed alongside the shared procedural mesh)
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
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // Position (location 2, after vertex position and normal)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Color (location 3)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

// ============================================================================
// UNIFORM DATA (camera and lighting)
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    _padding: u32,
}

impl Uniforms {
    fn new() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            _padding: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniform {
    direction: [f32; 3],
    _padding: u32,
    color: [f32; 3],
    _padding2: u32,
}

impl LightUniform {
    fn new() -> Self {
        Self {
            direction: [-0.3, -0.5, -0.6],
            _padding: 0,
            color: [1.0, 1.0, 1.0],
            _padding2: 0,
        }
    }
}

// ============================================================================
// PROCEDURAL MESH PIPELINE
// ============================================================================

/// Build a flat ground plane quad covering the full map area.
/// Vertices are at y=0 with an upward normal so they lit from above.
fn build_ground_plane_mesh(half_x: f32, half_z: f32) -> engine::mesh::RenderMesh {
    use engine::mesh::{GpuVertex, RenderMesh};
    let n = [0.0_f32, 1.0, 0.0];
    let vertices = vec![
        GpuVertex { position: [-half_x, 0.0, -half_z], normal: n },
        GpuVertex { position: [-half_x, 0.0,  half_z], normal: n },
        GpuVertex { position: [ half_x, 0.0,  half_z], normal: n },
        GpuVertex { position: [ half_x, 0.0, -half_z], normal: n },
    ];
    // CCW winding when viewed from above (+Y)
    let indices = vec![0u32, 1, 2, 0, 2, 3];
    RenderMesh { vertices, indices }
}

/// Build the test mesh: single vertex → Skin Modifier (cube) → Catmull-Clark ×2.
/// Returns a GPU-ready RenderMesh with smooth normals (98 verts, 576 indices).
fn build_procedural_sphere() -> engine::mesh::RenderMesh {
    use engine::{SkinGraph, skin_modifier, subdivide, triangulate_smooth};

    // Step 1: SkinGraph — single vertex at origin, radius 0.5
    let mut graph = SkinGraph::new();
    graph.add_node(Vec3::ZERO, 0.5);

    // Step 2: Skin Modifier → cube (8 verts, 6 quad faces)
    let cube_mesh = skin_modifier(&graph);

    // Step 3: Catmull-Clark ×2
    let subd_mesh = subdivide(&cube_mesh, 2);

    // Step 4: Triangulate + smooth normals
    let render_mesh = triangulate_smooth(&subd_mesh);

    render_mesh
}

// ============================================================================
// UNIT GROUPS  (test scene — 8 groups crossing the map)
// ============================================================================

struct UnitGroup {
    id: u32,
    color: [f32; 3],
    /// World-space spawn centre.
    start_world: glam::Vec3,
    /// World-space destination.
    goal_world: glam::Vec3,
    /// Pre-computed flowfield for this group's goal.
    flow_field: FlowField,
}

/// Build 2 head-on groups and compute their flowfields.
fn create_crossing_groups(nav_grid: &NavigationGrid) -> Vec<UnitGroup> {
    // (start_xz, goal_xz, rgb)
    let defs: &[([f32; 2], [f32; 2], [f32; 3])] = &[
        ([-35.0, 0.0], [35.0, 0.0], [1.00, 0.20, 0.20]), // W → E  Red
        ([ 35.0, 0.0], [-35.0, 0.0], [0.20, 0.50, 1.00]), // E → W  Blue
    ];

    defs.iter().enumerate().map(|(i, (start_xz, goal_xz, color))| {
        let start_world = glam::Vec3::new(start_xz[0], 0.5, start_xz[1]);
        let goal_world  = glam::Vec3::new(goal_xz[0],  0.0, goal_xz[1]);
        let flow_field  = compute_flowfield(nav_grid, goal_world);
        UnitGroup { id: i as u32, color: *color, start_world, goal_world, flow_field }
    }).collect()
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

    // Ground plane
    ground_vertex_buffer: wgpu::Buffer,
    ground_index_buffer: wgpu::Buffer,
    ground_instance_buffer: wgpu::Buffer,
    ground_num_indices: u32,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    light_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,

    // ECS World
    world: World,
    last_update: std::time::Instant,

    // Pathfinding
    nav_grid: NavigationGrid,
    groups: Vec<UnitGroup>,
    spatial_grid: SpatialGrid,

    // Camera & Input
    camera: RtsCamera,
    input: InputState,

    // Debug tracking
    frame_times: Vec<f32>,
    last_fps_update: std::time::Instant,
    fps_counter: u32,
    current_fps: u32,

    // Debug overlay (egui)
    debug_overlay: DebugOverlay,
    /// F4 toggle: draw avoidance-radius circles and velocity arrows per unit.
    debug_units_visible: bool,
}

impl State {
    fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    async fn new(window: &std::sync::Arc<winit::window::Window>) -> Self {
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
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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

        let light_uniform = LightUniform::new();
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Buffer"),
            contents: bytemuck::cast_slice(&[light_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let light_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("light_bind_group_layout"),
            });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &light_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
            label: Some("light_bind_group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&uniform_bind_group_layout, &light_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                // GpuVertex has the same layout as the old Vertex (locations 0, 1)
                buffers: &[GpuVertex::desc(), InstanceData::desc()],
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        use wgpu::util::DeviceExt;

        // Generate the procedural sphere mesh (single vertex → skin → CC×2)
        let render_mesh = build_procedural_sphere();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Procedural Vertex Buffer"),
            contents: render_mesh.vertex_bytes(),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Procedural Index Buffer"),
            contents: render_mesh.index_bytes(),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_indices = render_mesh.index_count() as u32;

        // Instance buffer for per-entity position+color (shared across all entities)
        let max_instances = 150;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (max_instances * std::mem::size_of::<InstanceData>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Ground plane buffers — oversized beyond camera bounds (±50) so edges are never visible
        let ground_mesh = build_ground_plane_mesh(100.0, 100.0);
        let ground_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ground Vertex Buffer"),
            contents: ground_mesh.vertex_bytes(),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ground_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ground Index Buffer"),
            contents: ground_mesh.index_bytes(),
            usage: wgpu::BufferUsages::INDEX,
        });
        let ground_num_indices = ground_mesh.index_count() as u32;
        let ground_instance = InstanceData {
            position: [0.0, 0.0, 0.0],
            _padding: 0.0,
            color: [0.25, 0.45, 0.25, 1.0],  // dark green
        };
        let ground_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ground Instance Buffer"),
            contents: bytemuck::cast_slice(&[ground_instance]),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let (depth_texture, depth_view) = Self::create_depth_texture(&device, &config);
        let debug_overlay = DebugOverlay::new(&window, &device, config.format);

        // Build navigation grid and compute flowfields for the crossing test.
        let nav_grid = NavigationGrid::new_open(GRID_WIDTH, GRID_HEIGHT);
        let groups = create_crossing_groups(&nav_grid);

        // Spatial grid for ORCA neighbour queries — 2-unit cells over the full map.
        use engine::navigation::WORLD_HALF;
        let spatial_grid = SpatialGrid::new(
            glam::Vec2::new(-WORLD_HALF, -WORLD_HALF),
            glam::Vec2::new( WORLD_HALF,  WORLD_HALF),
            2.0,
        );

        // ECS world — 2000 units in 8 crossing groups.
        let mut world = World::new();
        spawn_crossing_scene(&mut world, &groups);

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
            ground_vertex_buffer,
            ground_index_buffer,
            ground_instance_buffer,
            ground_num_indices,
            uniform_buffer,
            uniform_bind_group,
            light_bind_group,
            depth_texture,
            depth_view,
            world,
            last_update: std::time::Instant::now(),
            nav_grid,
            groups,
            spatial_grid,
            camera: RtsCamera::new(),
            input: {
                let mut input = InputState::new();
                input.window_size = (size.width, size.height);
                input
            },
            frame_times: Vec::with_capacity(100),
            last_fps_update: std::time::Instant::now(),
            fps_counter: 0,
            current_fps: 0,
            debug_overlay,
            debug_units_visible: false,
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, &self.config);
            self.depth_texture = depth_texture;
            self.depth_view = depth_view;
        }
    }

    fn update(&mut self) {
        let now = std::time::Instant::now();
        let dt = (now - self.last_update).as_secs_f32();
        self.last_update = now;

        if self.input.is_key_just_pressed(KeyCode::F3) {
            self.debug_overlay.toggle();
        }
        if self.input.is_key_just_pressed(KeyCode::F4) {
            self.debug_units_visible = !self.debug_units_visible;
        }

        self.camera.update(&self.input, dt);

        // ── Pathfinding + ORCA local avoidance ─────────────────────────────
        //
        // System order (matches pathfinding.md §"System Execution Order"):
        //   1. Collect unit snapshots from ECS  (read-only world access)
        //   2. Rebuild spatial grid
        //   3. Sample flowfield → desired_vel per unit
        //   4. ORCA → steering_vel per unit
        //   5. Write steering_vel back to Velocity  (write world access)
        //   6. Integrate positions

        // ── 1. Collect snapshots ─────────────────────────────────────────────
        let nav_grid = &self.nav_grid;
        let groups   = &self.groups;

        struct UnitSnap {
            entity:   bevy_ecs::entity::Entity,
            pos:      glam::Vec2,   // XZ
            vel:      glam::Vec2,   // XZ velocity from last frame
            radius:   f32,
            max_speed: f32,
            group_id: u32,
            arrived:  bool,
        }

        let snapshots: Vec<UnitSnap> = {
            let mut q = self.world.query::<(
                bevy_ecs::entity::Entity,
                &Transform,
                &Velocity,
                &UnitAgent,
                &GroupMembership,
            )>();
            q.iter(&self.world).map(|(entity, tf, vel, agent, gm)| {
                let p = tf.position;
                let v = vel.linear;
                let arrived = groups.get(gm.group_id as usize).map(|g| {
                    let dx = g.goal_world.x - p.x;
                    let dz = g.goal_world.z - p.z;
                    (dx * dx + dz * dz).sqrt() < ARRIVAL_RADIUS
                }).unwrap_or(false);
                UnitSnap {
                    entity,
                    pos:       glam::Vec2::new(p.x, p.z),
                    vel:       glam::Vec2::new(v.x, v.z),
                    radius:    agent.radius,
                    max_speed: agent.max_speed,
                    group_id:  gm.group_id,
                    arrived,
                }
            }).collect()
        };

        // ── 2. Rebuild spatial grid ──────────────────────────────────────────
        self.spatial_grid.clear();
        for (i, snap) in snapshots.iter().enumerate() {
            self.spatial_grid.insert(snap.pos, i);
        }

        // ── 3. Desired velocity from flowfield ───────────────────────────────
        let desired_vels: Vec<glam::Vec2> = snapshots.iter().map(|snap| {
            if snap.arrived { return glam::Vec2::ZERO; }
            let group = match groups.get(snap.group_id as usize) {
                Some(g) => g,
                None    => return glam::Vec2::ZERO,
            };
            let pos3 = Vec3::new(snap.pos.x, 0.5, snap.pos.y);
            let dir  = nav_grid
                .world_to_cell(pos3)
                .map(|cell| group.flow_field.sample_cell(cell))
                .unwrap_or(glam::Vec2::ZERO);

            if dir == glam::Vec2::ZERO {
                // At goal or unreachable cell — steer directly toward goal.
                let goal = glam::Vec2::new(group.goal_world.x, group.goal_world.z);
                (goal - snap.pos).normalize_or_zero() * snap.max_speed
            } else {
                dir * snap.max_speed
            }
        }).collect();

        // ── 4. ORCA ──────────────────────────────────────────────────────────
        let agent_snaps: Vec<AgentSnapshot> = snapshots.iter()
            .zip(desired_vels.iter())
            .map(|(snap, &dv)| AgentSnapshot {
                pos:         snap.pos,
                vel:         snap.vel,
                desired_vel: dv,
                radius:      snap.radius,
                max_speed:   snap.max_speed,
            })
            .collect();

        let inv_dt = if dt > 1e-4 { 1.0 / dt } else { 1000.0 };

        let steering_vels: Vec<Vec3> = (0..agent_snaps.len()).map(|i| {
            if snapshots[i].arrived { return Vec3::ZERO; }
            let v2 = compute_orca_velocity(
                &agent_snaps, i, &self.spatial_grid,
                ORCA_TIME_HORIZON, inv_dt,
            );
            Vec3::new(v2.x, 0.0, v2.y)
        }).collect();

        // ── 5. Write velocities ──────────────────────────────────────────────
        for (snap, &sv) in snapshots.iter().zip(steering_vels.iter()) {
            if let Some(mut velocity) = self.world.get_mut::<Velocity>(snap.entity) {
                velocity.linear = sv;
            }
        }

        // ── 6. Integrate positions ───────────────────────────────────────────
        {
            let mut query = self.world.query::<(&mut Transform, &Velocity)>();
            for (mut transform, velocity) in query.iter_mut(&mut self.world) {
                transform.position += velocity.linear * dt;
                transform.position.y = 0.5; // keep spheres on the ground plane
            }
        }
    }

    fn render(&mut self, window: &winit::window::Window) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Collect instance data from ECS
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
        if !instance_data.is_empty() {
            self.queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&instance_data[..instance_count]),
            );
        }

        // Update camera uniforms
        let aspect = self.size.width as f32 / self.size.height as f32;
        let uniforms = Uniforms {
            view_proj: self.camera.view_projection(aspect).to_cols_array_2d(),
            camera_pos: self.camera.camera_position().to_array(),
            _padding: 0,
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.05, b: 0.1, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.light_bind_group, &[]);
            // Draw ground plane (1 instance at origin)
            render_pass.set_vertex_buffer(0, self.ground_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.ground_instance_buffer.slice(..));
            render_pass.set_index_buffer(self.ground_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.ground_num_indices, 0, 0..1);

            // Draw spheres (instanced)
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..instance_count as u32);
        }

        // Debug overlay (egui) — F3 = stats panel, F4 = unit debug circles.
        // Run one egui frame covering both so we tessellate only once.
        if self.debug_overlay.visible || self.debug_units_visible {
            let ppp = window.scale_factor() as f32;
            let sw  = self.config.width  as f32;
            let sh  = self.config.height as f32;

            // ── F3 stats ────────────────────────────────────────────────────
            let stats: Option<DebugStats> = if self.debug_overlay.visible {
                let entity_count = self.world.query::<&Transform>().iter(&self.world).count();
                let (avg, mn, mx) = if !self.frame_times.is_empty() {
                    let avg = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
                    let mn  = self.frame_times.iter().copied().fold(f32::INFINITY, f32::min);
                    let mx  = self.frame_times.iter().copied().fold(0.0_f32, f32::max);
                    (avg, mn, mx)
                } else { (0.0, 0.0, 0.0) };

                Some(DebugStats {
                    fps: self.current_fps,
                    frame_time_avg_ms: avg * 1000.0,
                    frame_time_min_ms: mn  * 1000.0,
                    frame_time_max_ms: mx  * 1000.0,
                    entity_count,
                    draw_calls: 1,
                    resolution: (self.config.width, self.config.height),
                    camera_target: (self.camera.target().x, self.camera.target().y),
                    camera_distance: self.camera.distance(),
                    camera_zoom_pct: self.camera.zoom_fraction() * 100.0,
                })
            } else { None };

            // ── F4 unit debug ────────────────────────────────────────────────
            let unit_draws: Option<Vec<UnitDebugDraw>> = if self.debug_units_visible {
                let aspect = sw / sh;
                let vp = self.camera.view_projection(aspect);

                let draws = {
                    let mut q = self.world.query::<(&Transform, &Velocity, &UnitAgent)>();
                    q.iter(&self.world).filter_map(|(tf, vel, agent)| {
                        let p = tf.position;

                        let center = world_to_screen(p, vp, sw, sh, ppp)?;

                        // Velocity arrow: project 0.5 s of travel ahead.
                        let tip_world = p + vel.linear * 0.5;
                        let vel_tip = world_to_screen(tip_world, vp, sw, sh, ppp)
                            .unwrap_or(center);

                        // Avoidance radius: project a point one radius to the right.
                        let edge_world = Vec3::new(p.x + agent.radius, p.y, p.z);
                        let radius_px = world_to_screen(edge_world, vp, sw, sh, ppp)
                            .map(|ep| ((ep.x - center.x).powi(2) + (ep.y - center.y).powi(2)).sqrt())
                            .unwrap_or(5.0);

                        Some(UnitDebugDraw { pos: center, vel_tip, radius_px })
                    }).collect()
                };
                Some(draws)
            } else { None };

            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [self.config.width, self.config.height],
                pixels_per_point: ppp,
            };

            self.debug_overlay.render(
                &self.device,
                &self.queue,
                &mut encoder,
                window,
                &view,
                &screen_descriptor,
                stats.as_ref(),
                unit_draws.as_deref(),
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

// ============================================================================
// ENTITY SPAWNING
// ============================================================================

/// Spawn 75 units per group in a battle-line formation oriented perpendicular
/// to each group's travel direction so it reads clearly from any camera angle.
///
/// Layout: 15 wide (⟂ to travel) × 5 deep (∥ to travel) = 75 exactly.
fn spawn_crossing_scene(world: &mut World, groups: &[UnitGroup]) {
    const FORM_WIDE: u32 = 15; // units perpendicular to travel direction
    const FORM_DEEP: u32 = 5;  // units along travel direction
    const SPACING: f32 = 0.85; // world units between unit centres

    let half_w = FORM_WIDE as f32 * SPACING * 0.5;
    let half_d = FORM_DEEP as f32 * SPACING * 0.5;

    let mut total = 0u32;
    for group in groups {
        // Unit travel direction in the XZ plane (normalised).
        let to_goal = group.goal_world - group.start_world;
        let dist = (to_goal.x * to_goal.x + to_goal.z * to_goal.z).sqrt();
        let travel = if dist > 0.001 {
            glam::Vec2::new(to_goal.x / dist, to_goal.z / dist)
        } else {
            glam::Vec2::X
        };
        // Perpendicular to travel (rotate 90° CCW in XZ).
        let perp = glam::Vec2::new(-travel.y, travel.x);

        for d in 0..FORM_DEEP {
            for w in 0..FORM_WIDE {
                let offset_perp   = w as f32 * SPACING - half_w + SPACING * 0.5;
                let offset_travel = d as f32 * SPACING - half_d + SPACING * 0.5;

                let world_x = group.start_world.x
                    + perp.x * offset_perp
                    + travel.x * offset_travel;
                let world_z = group.start_world.z
                    + perp.y * offset_perp
                    + travel.y * offset_travel;

                world.spawn((
                    Transform::from_position(Vec3::new(
                        world_x.clamp(-49.0, 49.0),
                        0.5,
                        world_z.clamp(-49.0, 49.0),
                    )),
                    Velocity { linear: Vec3::ZERO },
                    EntityColor { r: group.color[0], g: group.color[1], b: group.color[2] },
                    GroupMembership { group_id: group.id },
                    UnitAgent { radius: UNIT_RADIUS, max_speed: UNIT_SPEED },
                ));
                total += 1;
            }
        }
    }
    println!("Crossing scene: {} units across {} groups", total, groups.len());
}

// ============================================================================
// MAIN
// ============================================================================

struct App {
    window: Option<std::sync::Arc<Window>>,
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("Flume Sugar - Procedural Mesh: Skin + Catmull-Clark ×2")
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));

            let window = std::sync::Arc::new(
                event_loop.create_window(window_attributes).unwrap()
            );

            let state = pollster::block_on(State::new(&window));
            self.window = Some(window);
            self.state = Some(state);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = &self.window else { return };
        let Some(state) = &mut self.state else { return };

        let _ = state.debug_overlay.handle_window_event(window, &event);
        state.input.process_event(&event);

        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event: KeyEvent {
                    state: ElementState::Pressed,
                    physical_key: PhysicalKey::Code(KeyCode::Escape),
                    ..
                },
                ..
            } => {
                event_loop.exit();
            }
            WindowEvent::Resized(physical_size) => {
                state.resize(physical_size);
            }
            WindowEvent::RedrawRequested => {
                let frame_start = std::time::Instant::now();

                state.update();
                match state.render(window) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                    Err(e) => eprintln!("{:?}", e),
                }

                let frame_time = frame_start.elapsed().as_secs_f32();
                state.frame_times.push(frame_time);
                if state.frame_times.len() > 100 {
                    state.frame_times.remove(0);
                }

                state.fps_counter += 1;
                let now = std::time::Instant::now();
                if (now - state.last_fps_update).as_secs_f32() >= 1.0 {
                    state.current_fps = state.fps_counter;
                    state.fps_counter = 0;
                    state.last_fps_update = now;

                    let avg_frame_time = if !state.frame_times.is_empty() {
                        state.frame_times.iter().sum::<f32>() / state.frame_times.len() as f32
                    } else {
                        0.0
                    };
                    let entity_count = state.world.query::<&Transform>().iter(&state.world).count();
                    window.set_title(&format!(
                        "Flume Sugar - {} FPS | {:.2} ms | {} entities",
                        state.current_fps,
                        avg_frame_time * 1000.0,
                        entity_count
                    ));
                }

                state.input.end_frame();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

/// Project a world-space position to egui screen points.
///
/// Returns `None` if the point is behind the camera or far off-screen.
/// `ppp` = pixels_per_point (window DPI scale factor).
fn world_to_screen(
    world: Vec3,
    vp: glam::Mat4,
    sw_px: f32,
    sh_px: f32,
    ppp: f32,
) -> Option<egui::Pos2> {
    let clip = vp * glam::Vec4::new(world.x, world.y, world.z, 1.0);
    if clip.w <= 0.0 { return None; }
    let nx = clip.x / clip.w;
    let ny = clip.y / clip.w;
    if nx < -1.2 || nx > 1.2 || ny < -1.2 || ny > 1.2 { return None; }
    let px = (nx + 1.0) * 0.5 * sw_px / ppp;
    let py = (1.0 - ny) * 0.5 * sh_px / ppp;
    Some(egui::pos2(px, py))
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        window: None,
        state: None,
    };

    event_loop.run_app(&mut app).unwrap();
}
