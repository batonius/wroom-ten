struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;
    var x: f32;
    var y: f32;
    switch (in_vertex_index) {
        case 0u: {
            x = -1.0;
            y = 1.0;
        }
        case 1u,4u: {
            x = 1.0;
            y = 1.0;
        }
        case 2u,3u: {
            x = -1.0;
            y = -1.0;
        }
        case 5u: {
            x = 1.0;
            y = -1.0;
        }
        default: {
            break;
        }
    }
    result.position = vec4<f32>(x, y, 0.0, 1.0);
    result.tex_coords = vec2<f32>((x + 1.0) / 2.0, (1.0 - y) / 2.0);
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(trace_for_point(vertex.tex_coords.xy), 1.0);
}

// Ray-tracing stars here
struct RayTracingParams {
    camera_pos: vec4<f32>,    
};

@group(0) @binding(0) var<uniform> params: RayTracingParams;

const CAMERA_X_AXIS: vec3<f32> = vec3<f32>(1.0, 0.0, 0.0);
const CAMERA_Y_AXIS: vec3<f32> = vec3<f32>(0.0, -1.0, 0.0);
const F32_MAX: f32 = 3.40282347E+38;
const EPSILON: f32 = 0.0001;
const SPHERE_R: f32 = 1.0;
const SPHERE_POS: vec3<f32> = vec3<f32>(0.0, 0.0, 10.0);

struct Ray {
    origin: vec3<f32>,
    dir: vec3<f32>,
}

fn make_start_ray_for_point(coord: vec2<f32>) -> Ray {
    var ray: Ray;
    ray.origin = params.camera_pos.xyz;
    let dir_point = (coord.x - 0.5) * CAMERA_X_AXIS + (coord.y - 0.5) * CAMERA_Y_AXIS;
    ray.dir = dir_point - params.camera_pos.xyz;
    return ray;
}

fn intersects_sphere(ray: Ray, pos: vec3<f32>, r: f32) -> f32 {
    let dir_squared = ray.dir * ray.dir;
    let delta = ray.origin - pos;
    let r_squared = r * r;
    let d = r_squared * (dir_squared.x + dir_squared.y + dir_squared.z)
        - pow(ray.dir.x * delta.y - ray.dir.y * delta.x, 2.0)
        - pow(ray.dir.x * delta.z - ray.dir.z * delta.x, 2.0)
        - pow(ray.dir.y * delta.z - ray.dir.z * delta.y, 2.0);
    if d < 0.00 {
        return F32_MAX;
    }
    let t1 = (-delta.x * ray.dir.x - delta.y * ray.dir.y - delta.z * ray.dir.z + sqrt(d))
            / (dir_squared.x + dir_squared.y + dir_squared.z);
    var t2 = F32_MAX;
    if d > EPSILON {
        t2 = (-delta.x * ray.dir.x - delta.y * ray.dir.y - delta.z * ray.dir.z - sqrt(d))
            / (dir_squared.x + dir_squared.y + dir_squared.z);
        
    }
    return min(t1, t2);
}

fn cast_ray(ray: Ray) -> vec3<f32> {
    var min_toi: f32 = 100000.0;
    var color = vec3<f32>(1.0, 1.0, 1.0);
    if ray.dir.x < 0.0 {
        let toi = (-4.0 - ray.origin.x) / ray.dir.x;
        if toi < min_toi {
            min_toi = toi;
            color = vec3<f32>(1.0, 0.0, 0.0);
        }
    } else {
        let toi = (4.0 - ray.origin.x) / ray.dir.x;
        if toi < min_toi {
            min_toi = toi;
            color = vec3<f32>(0.0, 1.0, 0.0);
        }
    }
    if ray.dir.y < 0.0 {
        let toi = (-2.0 - ray.origin.y) / ray.dir.y;
        if toi < min_toi {
            min_toi = toi;
            color = vec3<f32>(0.0, 0.0, 1.0);
        }
    } else {
        let toi = (2.0 - ray.origin.y) / ray.dir.y;
        if toi < min_toi {
            min_toi = toi;
            color = vec3<f32>(0.0, 1.0, 1.0);
        }
    }
    if ray.dir.z < 0.0 {
        let toi = (-0.0 - ray.origin.z) / ray.dir.z;
        if toi < min_toi {
            min_toi = toi;
            color = vec3<f32>(1.0, 0.0, 1.0);
        }
    } else {
        let toi = (16.0 - ray.origin.z) / ray.dir.z;
        if toi < min_toi {
            min_toi = toi;
            color = vec3<f32>(1.0, 1.0, 0.0);
        }
    }
    if (intersects_sphere(ray, SPHERE_POS, SPHERE_R) < min_toi) {
        color = vec3<f32>(1.0, 1.0, 1.0);
    }
    return color;
}

fn trace_for_point(coord: vec2<f32>) -> vec3<f32> {
    return cast_ray(make_start_ray_for_point(coord));
}
        
