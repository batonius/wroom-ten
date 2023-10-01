struct RayTracingParams {
    camera_pos: vec4<f32>,    
    aspect_ratio: f32,
    spheres_count: u32,
	time_elapsed: f32,
    _padding: f32,
};

struct Sphere {
    pos: vec3<f32>,
    r: f32,
    vel: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: RayTracingParams;
@group(0) @binding(1) var<storage, read> spheres_in: array<Sphere>;
@group(0) @binding(2) var<storage, read_write> spheres_out: array<Sphere>;

@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
	let index = global_invocation_id.x;
	spheres_out[index] = spheres_in[index];
	spheres_out[index].pos += spheres_in[index].vel.xyz * params.time_elapsed;
}
