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

// Ray-tracing starts here
struct RayTracingParams {
    camera_pos: vec4<f32>,    
    aspect_ratio: f32,
    spheres_count: u32,
    _padding: vec2<f32>,
};

struct Sphere {
    pos: vec3<f32>,
    r: f32,
    _vel: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: RayTracingParams;
@group(0) @binding(1) var<storage, read> spheres: array<Sphere>;

const CAMERA_X_AXIS: vec3<f32> = vec3<f32>(1.0, 0.0, 0.0);
const CAMERA_Y_AXIS: vec3<f32> = vec3<f32>(0.0, -1.0, 0.0);
const F32_MAX: f32 = 3.40282347E+38;
const EPSILON: f32 = 0.0001;
const REFLECTIONS_N: i32 = 5;
const MAX_TOI: f32 = 100000.0;

struct Ray {
    origin: vec3<f32>,
    dir: vec3<f32>,
}

fn make_start_ray_for_point(coord: vec2<f32>) -> Ray {
    var ray: Ray;
    ray.origin = params.camera_pos.xyz;
    let dir_point = (coord.x - 0.5) * CAMERA_X_AXIS + (coord.y - 0.5) * CAMERA_Y_AXIS / params.aspect_ratio;
    ray.dir = dir_point - params.camera_pos.xyz;
    return ray;
}

fn intersect_sphere(ray: Ray, pos: vec3<f32>, r: f32) -> f32 {
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

fn cast_ray(in_ray: Ray) -> vec3<f32> {
    var coef_color = vec3<f32>(1.0, 1.0, 1.0);
    var offset_color = vec3<f32>(0.0, 0.0, 0.0);
    var ray = in_ray;
    for (var i: i32 = 0; i < REFLECTIONS_N; i++) {
        var min_toi: f32 = MAX_TOI;
        var color = vec3<f32>(1.0, 0.0, 0.0);
        var normal = vec3<f32>(0.0, 0.0, 0.0);
        var with_sphere = false;
        if abs(ray.dir.x) > EPSILON {
            if ray.dir.x < 0.0 {
                let toi = (-4.0 - ray.origin.x) / ray.dir.x;
                if toi < min_toi {
                    min_toi = toi;
                    color = vec3<f32>(0.5, 0.0, 0.5);
                    normal = vec3<f32>(1.0, 0.0, 0.0);
                }
            } else {
                let toi = (4.0 - ray.origin.x) / ray.dir.x;
                if toi < min_toi {
                    min_toi = toi;
                    color = vec3<f32>(0.5, 0.0, 0.0);
                    normal = vec3<f32>(-1.0, 0.0, 0.0);
                }
            }
        }
        if abs(ray.dir.y) > EPSILON {
            if ray.dir.y < 0.0 {
                let toi = (-2.0 - ray.origin.y) / ray.dir.y;
                if toi < min_toi {
                    min_toi = toi;
                    color = vec3<f32>(0.0, 0.5, 0.5);
                    normal = vec3<f32>(0.0, 1.0, 0.0);
                }
            } else {
                let toi = (2.0 - ray.origin.y) / ray.dir.y;
                if toi < min_toi {
                    min_toi = toi;
                    color = vec3<f32>(0.5, 0.5, 0.0);
                    normal = vec3<f32>(0.0, -1.0, 0.0);
                }
            }
        }
        if abs(ray.dir.z) > EPSILON {
            if ray.dir.z < 0.0 {
                let toi = (-0.0 - ray.origin.z) / ray.dir.z;
                if toi < min_toi {
                    min_toi = toi;
                    color = vec3<f32>(0.0, 0.5, 0.0);
                    normal = vec3<f32>(0.0, 0.0, 1.0);
                }
            } else {
                let toi = (16.0 - ray.origin.z) / ray.dir.z;
                if toi < min_toi {
                    min_toi = toi;
                    color = vec3<f32>(0.0, 0.0, 0.5);
                    normal = vec3<f32>(0.0, 0.0, -1.0);
                }
            }
        }

        for (var sphere: u32 = 0u; sphere < params.spheres_count; sphere++) {
            let toi = intersect_sphere(ray, spheres[sphere].pos, spheres[sphere].r);
            if toi > EPSILON && toi < min_toi {
                min_toi = toi;
                let poi: vec3<f32> = ray.origin + ray.dir * toi;
                normal = normalize(poi - spheres[sphere].pos);
                color = vec3<f32>(0.1, 0.1, 0.1);
                with_sphere = true;
            }
        }

        if min_toi < MAX_TOI {
            let poi: vec3<f32> = ray.origin + ray.dir * min_toi;
            if !with_sphere {
                let offset_poi = (poi + vec3<f32>(1000.0, 1000.0, 1000.0)) * 1.5;
                let checkered : i32 = i32(round(offset_poi.x)) + i32(round(offset_poi.y)) + i32(round(offset_poi.z));
                if checkered % 2 == 0 {
                    offset_color += coef_color * color;
                    coef_color *= vec3<f32>(0.0, 0.0, 0.0);
                    break;
                }
            }
            let reflection_dir = ray.dir - 2.0 * dot(ray.dir, normal) * normal;
            ray.origin = poi;
            ray.dir = reflection_dir;
            offset_color += coef_color * color;
            if with_sphere {
                coef_color *= vec3<f32>(0.7, 0.7, 0.7);
            } else {
                coef_color *= vec3<f32>(0.3, 0.3, 0.3);
            }
        } else {
            break;
        }
    }
    return offset_color + coef_color;
}

fn trace_for_point(coord: vec2<f32>) -> vec3<f32> {
    return cast_ray(make_start_ray_for_point(coord));
}
        
