// WGSL shader for the progress bar animated "liquid lava" fill.
//
// Draws a rectangular fill region with domain-warped animated noise:
// darker base with brighter molten highlights. The base color is passed
// via uniforms (silver normally, orange on press, red during seek), and
// `intensity` brightens the whole fill (hover).

struct Uniforms {
    /// Expanded callback rect in physical pixels (min_x, min_y, max_x, max_y)
    viewport: vec4<f32>,
    /// Fill rect in physical pixels (min_x, min_y, max_x, max_y)
    fill_rect: vec4<f32>,
    /// Base fill color (RGBA)
    color: vec4<f32>,
    /// Animation time
    time: f32,
    /// Brightness multiplier (Idle 1.0, Hovered ~1.35)
    intensity: f32,
    /// Uniform aging of the lava pattern (longer = faster)
    speed: f32,
    /// Unused, keep struct 16-byte aligned
    _pad: f32,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VSOut {
    let pos = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    var out: VSOut;
    out.pos = vec4<f32>(pos[vertex_index] * 2.0 - 1.0, 0.0, 1.0);
    out.uv = pos[vertex_index];
    return out;
}

fn hash3(p: vec3<f32>) -> f32 {
    let s = dot(p, vec3<f32>(127.1, 311.7, 74.7));
    return fract(sin(s) * 43758.5453123);
}

fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(
            mix(hash3(i + vec3<f32>(0.0, 0.0, 0.0)), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

fn fbm(p: vec3<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var pos = p;
    var i = 0u;
    while (i < 4u) {
        v += a * noise3d(pos);
        pos = pos * 2.1;
        a *= 0.5;
        i += 1u;
    }
    return v;
}

@fragment
fn fs_main(input: VSOut) -> @location(0) vec4<f32> {
    let pixel = u.viewport.xy + input.uv * (u.viewport.zw - u.viewport.xy);

    // Fill rect bounds (physical px)
    let size = u.fill_rect.zw - u.fill_rect.xy;
    let local = (pixel - u.fill_rect.xy) / max(size, vec2<f32>(1.0, 1.0));

    // Domain-warped liquid lava pattern
    let t = u.time * u.speed;
    let q = vec3<f32>(pixel.x * 0.09, pixel.y * 0.18, t * 0.6);
    let warp = fbm(q + vec3<f32>(0.0, 0.0, t * 0.9));
    let lava = fbm(q + vec3<f32>(warp * 2.2, warp * 1.4, t * 0.35));

    // Base -> mid -> hot highlight gradient
    let base = u.color.rgb * 0.35;
    let mid = u.color.rgb;
    let hot = mix(u.color.rgb, vec3<f32>(1.0, 1.0, 1.0), 0.75);

    var rgb = mix(base, mid, smoothstep(0.15, 0.65, lava));
    rgb = mix(rgb, hot, smoothstep(0.62, 0.92, lava));

    // Subtle vertical sheen so the bar reads as a tube
    let sheen = 1.0 - abs(local.y - 0.5) * 1.1;
    rgb *= 0.82 + 0.18 * sheen;

    rgb *= u.intensity;

    return vec4<f32>(rgb, u.color.a);
}
