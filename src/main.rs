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
use glam::{Mat4, Vec3, UVec2};
use bevy_ecs::prelude::*;
use engine::{Transform, Color as EntityColor, Velocity, GroupMembership, UnitAgent, FormationOffset};
use engine::{NavigationGrid, FlowField, DensityMap, compute_flowfield, compute_flowfield_with_density, GRID_WIDTH, GRID_HEIGHT};
use engine::{AgentSnapshot, SpatialGrid, compute_orca_velocity};
use engine::camera::RtsCamera;
use engine::debug_overlay::{DebugOverlay, DebugStats, UnitDebugDraw, FlowfieldArrowDraw, DensityCell};
use engine::input::InputState;
use engine::mesh::GpuVertex;
use egui;

/// Movement speed for all units (world units per second).
const UNIT_SPEED: f32 = 2.5;
/// Physical collision radius of each unit (matches the procedural sphere mesh).
const UNIT_RADIUS: f32 = 0.5;
/// Units within this world-space distance of their goal are considered arrived.
const ARRIVAL_RADIUS: f32 = 1.5;
/// ORCA look-ahead window (seconds).  Shorter = more direct movement; longer = smoother lanes.
const ORCA_TIME_HORIZON: f32 = 0.8;
/// Density surcharge weight added to each cell's flowfield cost per unit present.
/// Controls how aggressively units spread across available corridor width.
/// See pathfinding.md §"The Novel Part: Density Feedback Cost".
const DENSITY_WEIGHT: f32 = 0.15;
/// Rebuild the density map and recompute flowfields every this many frames.
/// At 60 FPS this is ~133 ms — frequent enough to feel responsive,
/// cheap enough to stay well within the 16 ms frame budget.
const DENSITY_UPDATE_INTERVAL: u32 = 8;

// ── Sprint 4: Formation + Arrival ──────────────────────────────────────────

/// Formation columns (perpendicular to travel). Shared by spawner and formation system.
const FORM_WIDE: u32 = 15;
/// World-unit spacing between formation slots and between spawned unit centres.
/// Units have diameter 1.0, so 2.0 here = 1.0 unit of clear gap between surfaces.
const FORM_SPACING: f32 = 1.5;
/// Slot-pull strength: max fraction of max_speed applied toward the formation slot.
/// Higher than before because intra-group ORCA is off — the slot pull is the only
/// force maintaining formation shape and driving post-crossing reformation.
const SLOT_PULL: f32 = 0.50;
/// Distance (world units) at which the slot-pull spring reaches full strength.
const SLOT_PULL_DIST: f32 = 6.0;
/// Fraction of group units that must reach the goal before the group is disbanded.
const ARRIVAL_FRACTION: f32 = 0.90;

// ── Sprint 5: Formation Reformation (catch-up / wait) ─────────────────────

/// Forward slot-error (world units along travel dir) above which a unit is "lagging".
/// Lagging units exceed normal max_speed to sprint back to their slot.
const REFORM_LAG_THRESHOLD: f32 = 1.5;
/// Forward slot-error below this marks a unit as "leading" (it is ahead of its slot).
const REFORM_LEAD_THRESHOLD: f32 = -1.0;
/// Group max-lag (world units) at which leading units begin slowing to wait.
/// Ramps linearly to a full stop 2 world units later.
const REFORM_WAIT_TRIGGER: f32 = 3.0;
/// Speed multiplier for lagging units during reformation.
/// 2× normal speed means a unit 4–5 units behind its slot closes the gap in under one second.
const CATCHUP_SPEED_MULT: f32 = 2.0;

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

/// Per-group formation state: arrival tracking and travel direction.
///
/// Slot positions are no longer stored here — each unit carries a fixed
/// `FormationOffset` component and its slot target is recomputed each frame
/// as `group_centroid + formation_offset`.
struct GroupFormation {
    /// True once enough units have reached the goal — group is disbanded (idle).
    arrived: bool,
    /// Last known travel direction (flowfield dir at group centroid). Vec2::ZERO until first update.
    travel_dir: glam::Vec2,
}

impl GroupFormation {
    fn new() -> Self {
        Self {
            arrived: false,
            travel_dir: glam::Vec2::ZERO,
        }
    }
}

/// Build 2 head-on groups and compute their flowfields.
fn create_crossing_groups(nav_grid: &NavigationGrid) -> Vec<UnitGroup> {
    // (start_xz, goal_xz, rgb)
    let defs: &[([f32; 2], [f32; 2], [f32; 3])] = &[
        ([-35.0, -35.0], [ 35.0,  35.0], [1.00, 0.20, 0.20]), // NW → SE  Red
        ([ 35.0,  35.0], [-35.0, -35.0], [0.20, 0.50, 1.00]), // SE → NW  Blue
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
    formations: Vec<GroupFormation>,
    spatial_grid: SpatialGrid,
    density_map: DensityMap,
    frame_count: u32,

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
    /// F5 toggle: draw flowfield arrows and density heatmap.
    debug_flowfield_visible: bool,
    /// Milliseconds spent on the last flowfield recomputation pass.
    pathfinding_last_ms: f32,
    /// Running total of flowfield recomputes since startup.
    flowfield_recompute_count: u32,
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
            formations: (0..groups.len()).map(|_| GroupFormation::new()).collect(),
            nav_grid,
            groups,
            spatial_grid,
            density_map: DensityMap::new(GRID_WIDTH, GRID_HEIGHT),
            frame_count: 0,
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
            debug_flowfield_visible: false,
            pathfinding_last_ms: 0.0,
            flowfield_recompute_count: 0,
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
        if self.input.is_key_just_pressed(KeyCode::F5) {
            self.debug_flowfield_visible = !self.debug_flowfield_visible;
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
            entity:           bevy_ecs::entity::Entity,
            pos:              glam::Vec2,   // XZ
            vel:              glam::Vec2,   // XZ velocity from last frame
            radius:           f32,
            max_speed:        f32,
            group_id:         u32,
            arrived:          bool,
            formation_offset: glam::Vec2,  // fixed spawn-time offset from group centroid
        }

        let snapshots: Vec<UnitSnap> = {
            let mut q = self.world.query::<(
                bevy_ecs::entity::Entity,
                &Transform,
                &Velocity,
                &UnitAgent,
                &GroupMembership,
                &FormationOffset,
            )>();
            q.iter(&self.world).map(|(entity, tf, vel, agent, gm, fo)| {
                let p = tf.position;
                let v = vel.linear;
                let arrived = groups.get(gm.group_id as usize).map(|g| {
                    // Arrive at the unit's own formation slot at the goal, not at
                    // the goal point itself — otherwise all units clump at one spot.
                    let dx = g.goal_world.x + fo.offset.x - p.x;
                    let dz = g.goal_world.z + fo.offset.y - p.z;
                    (dx * dx + dz * dz).sqrt() < ARRIVAL_RADIUS
                }).unwrap_or(false);
                UnitSnap {
                    entity,
                    pos:              glam::Vec2::new(p.x, p.z),
                    vel:              glam::Vec2::new(v.x, v.z),
                    radius:           agent.radius,
                    max_speed:        agent.max_speed,
                    group_id:         gm.group_id,
                    arrived,
                    formation_offset: fo.offset,
                }
            }).collect()
        };

        // ── 2. Rebuild spatial grid ──────────────────────────────────────────
        self.spatial_grid.clear();
        for (i, snap) in snapshots.iter().enumerate() {
            self.spatial_grid.insert(snap.pos, i);
        }

        // ── 3a. Per-group data collection ────────────────────────────────────
        // Build per-group unit lists (for slot assignment) and centroids
        // (for formation centre / fallback slot target).
        // Only non-arrived units are included in the active lists.
        let num_groups = groups.len();
        let mut group_unit_data:     Vec<Vec<(bevy_ecs::entity::Entity, glam::Vec2)>> =
            vec![Vec::new(); num_groups];
        let mut group_centroid_sum   = vec![glam::Vec2::ZERO; num_groups];
        let mut group_arrived_count  = vec![0u32; num_groups];
        let mut group_total_count    = vec![0u32; num_groups];

        for snap in &snapshots {
            let gid = snap.group_id as usize;
            if gid < num_groups {
                group_total_count[gid] += 1;
                if snap.arrived {
                    group_arrived_count[gid] += 1;
                } else {
                    group_unit_data[gid].push((snap.entity, snap.pos));
                    group_centroid_sum[gid] += snap.pos;
                }
            }
        }
        let group_centroids: Vec<glam::Vec2> = group_centroid_sum.iter()
            .zip(group_unit_data.iter())
            .map(|(&sum, units)| {
                if !units.is_empty() { sum / units.len() as f32 } else { glam::Vec2::ZERO }
            })
            .collect();

        // ── 3b. Formation update (arrival + travel direction) ─────────────────
        //
        // Slot targets are no longer reassigned here — each unit carries a
        // fixed `FormationOffset` and its slot is recomputed every frame as
        // `group_centroid + formation_offset` (see 3c below).
        for gid in 0..num_groups {
            if self.formations[gid].arrived { continue; }

            // ── Arrival check: disband when ARRIVAL_FRACTION of units at goal ─
            let total     = group_total_count[gid];
            let n_arrived = group_arrived_count[gid];
            if total > 0 && n_arrived as f32 >= total as f32 * ARRIVAL_FRACTION {
                self.formations[gid].arrived = true;
                println!(
                    "Group {} arrived ({}/{} units at goal).",
                    gid, n_arrived, total,
                );
                continue;
            }

            if group_unit_data[gid].is_empty() { continue; }

            let centroid_xz   = group_centroids[gid];
            let centroid_world = Vec3::new(centroid_xz.x, 0.5, centroid_xz.y);

            // ── Update travel direction from flowfield at group centroid ───────
            let flow_at_centroid = self.nav_grid
                .world_to_cell(centroid_world)
                .map(|cell| self.groups[gid].flow_field.sample_cell(cell))
                .unwrap_or(glam::Vec2::ZERO);

            if flow_at_centroid.length_squared() > 0.01 {
                self.formations[gid].travel_dir = flow_at_centroid.normalize();
            } else if self.formations[gid].travel_dir.length_squared() < 0.01 {
                let goal  = self.groups[gid].goal_world;
                let to_goal = glam::Vec2::new(
                    goal.x - centroid_xz.x,
                    goal.z - centroid_xz.y,
                );
                if to_goal.length_squared() > 0.01 {
                    self.formations[gid].travel_dir = to_goal.normalize();
                }
            }
        }

        // ── 3b.5. Per-group formation disruption ─────────────────────────────
        //
        // Project each unit's slot error onto the group travel direction.
        // Positive forward_err = unit is behind its slot (lagging).
        // We track the worst lag per group; leaders use it to decide how much to slow.
        let formations = &self.formations;
        let group_max_lag: Vec<f32> = {
            let mut lag = vec![0.0f32; num_groups];
            for snap in &snapshots {
                if snap.arrived { continue; }
                let gid = snap.group_id as usize;
                if gid >= num_groups { continue; }
                if formations[gid].arrived { continue; }
                let travel_dir = formations[gid].travel_dir;
                if travel_dir.length_squared() < 0.01 { continue; }
                let centroid    = group_centroids[gid];
                let slot_target = centroid + snap.formation_offset;
                let forward_err = (slot_target - snap.pos).dot(travel_dir);
                if forward_err > lag[gid] { lag[gid] = forward_err; }
            }
            lag
        };

        // ── 3c. Desired velocity = flowfield + slot pull ──────────────────────
        //
        // Replaces the old boids-centroid cohesion with per-unit slot targets.
        // The slot pull is a soft spring (SLOT_PULL fraction of max_speed), weaker
        // than ORCA so units briefly vacate slots during collisions and drift back.
        //
        // Reformation modifiers (Sprint 5):
        //   Lagging units (forward slot-error > REFORM_LAG_THRESHOLD) get
        //   CATCHUP_SPEED_MULT × max_speed so they sprint back to their slot.
        //   Leading units (forward slot-error < REFORM_LEAD_THRESHOLD) ramp
        //   their flowfield contribution to zero once the group's worst lag
        //   exceeds REFORM_WAIT_TRIGGER, making them wait for stragglers.
        //   The same effective speed is passed to ORCA so the solver doesn't
        //   clamp the boost away.
        let (desired_vels, effective_speeds): (Vec<glam::Vec2>, Vec<f32>) =
            snapshots.iter().map(|snap| {
            let gid = snap.group_id as usize;

            // Arrived units (individually or whole group disbanded) — stop.
            if snap.arrived { return (glam::Vec2::ZERO, snap.max_speed); }
            if formations.get(gid).map(|f| f.arrived).unwrap_or(false) {
                return (glam::Vec2::ZERO, snap.max_speed);
            }

            let group = match groups.get(gid) {
                Some(g) => g,
                None    => return (glam::Vec2::ZERO, snap.max_speed),
            };

            // ── Reformation: classify this unit and derive speed modifiers ────
            //
            // forward_err > 0  →  slot is ahead of unit  →  unit is lagging
            // forward_err < 0  →  slot is behind unit    →  unit is leading
            let travel_dir = formations.get(gid).map(|f| f.travel_dir).unwrap_or(glam::Vec2::ZERO);
            let max_lag    = group_max_lag.get(gid).copied().unwrap_or(0.0);
            let (effective_max_speed, wait_factor) = if travel_dir.length_squared() > 0.01 {
                let centroid    = group_centroids.get(gid).copied().unwrap_or(snap.pos);
                let slot_target = centroid + snap.formation_offset;
                let forward_err = (slot_target - snap.pos).dot(travel_dir);

                // Lagging: sprint at up to CATCHUP_SPEED_MULT × normal speed.
                let ems = if forward_err > REFORM_LAG_THRESHOLD {
                    snap.max_speed * CATCHUP_SPEED_MULT
                } else {
                    snap.max_speed
                };

                // Leading while the group is disrupted: scale flowfield contribution
                // toward zero so the unit waits for stragglers. Ramps from 1.0 at
                // REFORM_WAIT_TRIGGER to a full stop 2 world units later.
                let wf = if forward_err < REFORM_LEAD_THRESHOLD && max_lag > REFORM_WAIT_TRIGGER {
                    let t = ((max_lag - REFORM_WAIT_TRIGGER) / 2.0).clamp(0.0, 1.0);
                    1.0_f32 - t
                } else {
                    1.0_f32
                };

                (ems, wf)
            } else {
                (snap.max_speed, 1.0_f32)
            };

            // Flowfield direction (scaled by wait_factor for leading units).
            let pos3 = Vec3::new(snap.pos.x, 0.5, snap.pos.y);
            let dir  = nav_grid
                .world_to_cell(pos3)
                .map(|cell| group.flow_field.sample_cell(cell))
                .unwrap_or(glam::Vec2::ZERO);
            let flowfield_vel = if dir == glam::Vec2::ZERO {
                let goal = glam::Vec2::new(group.goal_world.x, group.goal_world.z);
                (goal - snap.pos).normalize_or_zero() * effective_max_speed * wait_factor
            } else {
                dir * effective_max_speed * wait_factor
            };

            // Slot pull: full 2D spring toward the unit's fixed formation slot.
            //
            // Slot target = group centroid + spawn-time formation offset.
            // Because intra-group ORCA is disabled, the only thing that displaces
            // a unit from its slot is the opposing group crossing through. The full
            // 2D pull (both lateral and forward/backward) means:
            //   - Before crossing: near-zero error → near-zero pull → tight formation.
            //   - During crossing: inter-group ORCA competes with pull → fluid deformation.
            //   - After crossing:  no more ORCA → slot pull drives reformation.
            // Uses effective_max_speed so lagging units also get the boost on the pull.
            let slot_pull_vel = if let Some(_formation) = formations.get(gid) {
                let centroid    = group_centroids.get(gid).copied().unwrap_or(snap.pos);
                let slot_target = centroid + snap.formation_offset;
                let to_slot     = slot_target - snap.pos;
                let slot_dist   = to_slot.length();
                if slot_dist > 0.05 {
                    let t = (slot_dist / SLOT_PULL_DIST).min(1.0);
                    to_slot / slot_dist * effective_max_speed * t * SLOT_PULL
                } else {
                    glam::Vec2::ZERO
                }
            } else {
                glam::Vec2::ZERO
            };

            let desired_vel = (flowfield_vel + slot_pull_vel).clamp_length_max(effective_max_speed);
            (desired_vel, effective_max_speed)
        }).unzip();

        // ── 4. ORCA ──────────────────────────────────────────────────────────
        let agent_snaps: Vec<AgentSnapshot> = snapshots.iter()
            .zip(desired_vels.iter())
            .zip(effective_speeds.iter())
            .map(|((snap, &dv), &ems)| AgentSnapshot {
                pos:         snap.pos,
                vel:         snap.vel,
                desired_vel: dv,
                radius:      snap.radius,
                max_speed:   ems,   // boosted for laggers so ORCA doesn't clamp the sprint
                group_id:    snap.group_id,
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

        // ── 7. Density-feedback flowfield recomputation (every N frames) ─────
        //
        // Rebuild the density map from current unit positions, then recompute
        // each group's flowfield with a density surcharge so units spread across
        // the full corridor width instead of queuing in a single-file line.
        //
        // Runs on fresh post-integration positions so the surcharge reflects
        // where units actually are after this frame's movement.
        // See pathfinding.md §"Layer 2: Group Flowfield / The Novel Part".
        self.frame_count += 1;
        if self.frame_count % DENSITY_UPDATE_INTERVAL == 0 {
            let t_pf = std::time::Instant::now();

            self.density_map.clear();
            {
                let mut q = self.world.query::<&Transform>();
                for tf in q.iter(&self.world) {
                    self.density_map.add_unit(tf.position);
                }
            }
            for group in &mut self.groups {
                group.flow_field = compute_flowfield_with_density(
                    &self.nav_grid,
                    group.goal_world,
                    &self.density_map,
                    DENSITY_WEIGHT,
                );
            }

            self.pathfinding_last_ms = t_pf.elapsed().as_secs_f32() * 1000.0;
            self.flowfield_recompute_count += 1;
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

        // Debug overlay (egui) — F3 = stats, F4 = unit circles, F5 = flowfield/density.
        // Run one egui frame covering all active layers so we tessellate only once.
        if self.debug_overlay.visible || self.debug_units_visible || self.debug_flowfield_visible {
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
                    pathfinding_ms: self.pathfinding_last_ms,
                    flowfield_recomputes: self.flowfield_recompute_count,
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

            // ── F5 flowfield arrows + density heatmap ────────────────────────
            //
            // Flowfield: sample every 3rd cell of group 0's flowfield.
            // Density:   draw all cells that have at least one unit.
            let (flowfield_arrows, density_cells): (Option<Vec<FlowfieldArrowDraw>>, Option<Vec<DensityCell>>) =
                if self.debug_flowfield_visible {
                    // Arrow length in world units — 40% of one cell so tips stay inside the cell.
                    const ARROW_LEN: f32 = 0.40;
                    const STEP: u32 = 3;
                    const MAX_DENSITY: f32 = 5.0;

                    let vp = self.camera.view_projection(sw / sh);

                    let mut arrows: Vec<FlowfieldArrowDraw> = Vec::new();
                    let flow = &self.groups[0].flow_field;
                    for cz in (0..GRID_HEIGHT).step_by(STEP as usize) {
                        for cx in (0..GRID_WIDTH).step_by(STEP as usize) {
                            let cell = UVec2::new(cx, cz);
                            let dir = flow.sample_cell(cell);
                            if dir == glam::Vec2::ZERO { continue; }

                            let center = self.nav_grid.cell_center(cell);
                            let tip = Vec3::new(
                                center.x + dir.x * ARROW_LEN,
                                center.y,
                                center.z + dir.y * ARROW_LEN,
                            );
                            if let (Some(from), Some(to)) = (
                                world_to_screen(center, vp, sw, sh, ppp),
                                world_to_screen(tip, vp, sw, sh, ppp),
                            ) {
                                arrows.push(FlowfieldArrowDraw { from, to });
                            }
                        }
                    }

                    let mut cells: Vec<DensityCell> = Vec::new();
                    for cz in 0..GRID_HEIGHT {
                        for cx in 0..GRID_WIDTH {
                            let cell = UVec2::new(cx, cz);
                            let count = self.density_map.get(cell);
                            if count < 0.5 { continue; }

                            let center_w = self.nav_grid.cell_center(cell);
                            let edge_w   = Vec3::new(center_w.x + 0.5, center_w.y, center_w.z);
                            if let Some(center_s) = world_to_screen(center_w, vp, sw, sh, ppp) {
                                let size_px = world_to_screen(edge_w, vp, sw, sh, ppp)
                                    .map(|ep| {
                                        let dx = ep.x - center_s.x;
                                        let dy = ep.y - center_s.y;
                                        (dx * dx + dy * dy).sqrt() * 2.0
                                    })
                                    .unwrap_or(4.0);
                                let intensity = (count / MAX_DENSITY).min(1.0);
                                cells.push(DensityCell { center: center_s, size_px, intensity });
                            }
                        }
                    }

                    (Some(arrows), Some(cells))
                } else {
                    (None, None)
                };

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
                flowfield_arrows.as_deref(),
                density_cells.as_deref(),
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
///
/// Each unit receives a `FormationOffset` equal to its spawn position minus the
/// group centroid (`start_world.xz`). The formation system uses this fixed
/// offset every frame so the group keeps its original grid shape throughout.
fn spawn_crossing_scene(world: &mut World, groups: &[UnitGroup]) {
    // FORM_WIDE and FORM_SPACING are module-level constants shared with the formation system.
    const FORM_DEEP: u32 = 5; // rows along travel direction

    let half_w = (FORM_WIDE  as f32 - 1.0) * FORM_SPACING * 0.5;
    let half_d = (FORM_DEEP  as f32 - 1.0) * FORM_SPACING * 0.5;

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

        // The formation centroid coincides with start_world.
        let centroid_x = group.start_world.x;
        let centroid_z = group.start_world.z;

        for d in 0..FORM_DEEP {
            for w in 0..FORM_WIDE {
                let offset_perp   = w as f32 * FORM_SPACING - half_w;
                let offset_travel = d as f32 * FORM_SPACING - half_d;

                let world_x = centroid_x
                    + perp.x   * offset_perp
                    + travel.x * offset_travel;
                let world_z = centroid_z
                    + perp.y   * offset_perp
                    + travel.y * offset_travel;

                let spawn_x = world_x.clamp(-49.0, 49.0);
                let spawn_z = world_z.clamp(-49.0, 49.0);

                // Fixed offset from group centroid — preserved for the whole journey.
                let formation_offset = glam::Vec2::new(
                    spawn_x - centroid_x,
                    spawn_z - centroid_z,
                );

                world.spawn((
                    Transform::from_position(Vec3::new(spawn_x, 0.5, spawn_z)),
                    Velocity { linear: Vec3::ZERO },
                    EntityColor { r: group.color[0], g: group.color[1], b: group.color[2] },
                    GroupMembership { group_id: group.id },
                    UnitAgent { radius: UNIT_RADIUS, max_speed: UNIT_SPEED },
                    FormationOffset { offset: formation_offset },
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
