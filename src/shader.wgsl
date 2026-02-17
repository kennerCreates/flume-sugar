// Shader for rendering ECS entities
// Each entity has its own transform and color

// Uniform buffer with view-projection, model, and color
struct Uniforms {
    view_proj: mat4x4<f32>,  // Camera view-projection matrix
    model: mat4x4<f32>,      // Entity transform (position, rotation, scale)
    color: vec4<f32>,        // Entity color (RGBA)
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Vertex input from mesh
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,  // Vertex color (not used currently, but kept for compatibility)
}

// Output from vertex shader to fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;

    // Transform: model space -> world space -> view space -> clip space
    let world_position = uniforms.model * vec4<f32>(input.position, 1.0);
    output.clip_position = uniforms.view_proj * world_position;

    // Use entity color
    output.color = uniforms.color;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
