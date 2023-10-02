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
    vel: vec3<f32>,
    _padding: f32,
    _color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: RayTracingParams;
@group(0) @binding(1) var<storage, read> spheres_in: array<Sphere>;
@group(0) @binding(2) var<storage, read_write> spheres_out: array<Sphere>;

@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) global_invocation_id: vec3<u32>) {
    let index = global_invocation_id.x;
    spheres_out[index] = spheres_in[index];
    spheres_out[index].pos += spheres_in[index].vel * params.time_elapsed;

    var collided = false;
    var normal = vec3<f32>(0.0, 0.0, 0.0);

    if (4.0 - spheres_out[index].pos.x) < spheres_out[index].r {
        collided = true;
        normal = vec3<f32>(-1.0, 0.0, 0.0);
    } else if (4.0 + spheres_out[index].pos.x) < spheres_out[index].r {
        collided = true;
        normal = vec3<f32>(1.0, 0.0, 0.0);
    } else if (2.0 - spheres_out[index].pos.y) < spheres_out[index].r {
        collided = true;
        normal = vec3<f32>(0.0, -1.0, 0.0);
    } else if (2.0 + spheres_out[index].pos.y) < spheres_out[index].r {
        collided = true;
        normal = vec3<f32>(0.0, 1.0, 0.0);
    } else if (16.0 - spheres_out[index].pos.z) < spheres_out[index].r {
        collided = true;
        normal = vec3<f32>(0.0, 0.0, -1.0);
    } else if (spheres_out[index].pos.z) < spheres_out[index].r {
        collided = true;
        normal = vec3<f32>(0.0, 0.0, 1.0);
    } else {
        for (var sphere: u32 = 0u; sphere < params.spheres_count; sphere++) {
            if sphere == index {
                continue;
            }
            let sphere_pos = spheres_in[sphere].pos + spheres_in[sphere].vel * params.time_elapsed;
            let diff = spheres_out[index].pos - sphere_pos;
            if length(diff) < (spheres_out[index].r + spheres_in[sphere].r) {
                collided = true;
                normal = normalize(diff);
                spheres_out[index].pos = sphere_pos + normal * (spheres_out[index].r + spheres_in[sphere].r);
                break;
            }
        }
    }
    if collided {
        spheres_out[index].vel -= normal * dot(spheres_out[index].vel, normal) * 2.0;
    }
}
