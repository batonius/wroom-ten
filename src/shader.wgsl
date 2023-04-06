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
    return vec4<f32>(vertex.tex_coords.x, (vertex.tex_coords.x + vertex.tex_coords.y) / 2.0, vertex.tex_coords.y, 1.0);
}
        
