// Instanced rendering shader with Blinn-Phong lighting
// Each instance has its own position and color

// Camera uniforms (bind group 0)
struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Light uniforms (bind group 1)
struct Light {
    direction: vec3<f32>,
    color: vec3<f32>,
}

@group(1) @binding(0)
var<uniform> light: Light;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct InstanceInput {
    @location(2) instance_position: vec3<f32>,
    @location(3) instance_color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

@vertex
fn vs_main(
    vertex: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Offset vertex position by instance position
    let world_position = vertex.position + instance.instance_position;
    out.clip_position = uniforms.view_proj * vec4<f32>(world_position, 1.0);
    out.world_position = world_position;
    out.world_normal = vertex.normal;  // Cubes don't rotate, so normal is unchanged
    out.color = instance.instance_color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Normalize inputs
    let normal = normalize(in.world_normal);
    let light_dir = normalize(-light.direction);  // Negate because direction points away from light

    // Ambient lighting
    let ambient_strength = 0.1;
    let ambient = light.color * ambient_strength;

    // Diffuse lighting
    let diff = max(dot(normal, light_dir), 0.0);
    let diffuse = light.color * diff;

    // Specular lighting (Blinn-Phong)
    let view_dir = normalize(uniforms.camera_pos - in.world_position);
    let half_dir = normalize(view_dir + light_dir);
    let spec = pow(max(dot(normal, half_dir), 0.0), 32.0);  // 32 = shininess
    let specular_strength = 0.5;
    let specular = light.color * spec * specular_strength;

    // Combine lighting with object color
    let lighting = ambient + diffuse + specular;
    let result = lighting * in.color.rgb;

    return vec4<f32>(result, in.color.a);
}
