// Prism's viewport shaders.
//
// Positions arrive as two f32 parts (high and low) of the f64 coordinate.
// Subtracting the eye's two parts first keeps everything relative to the
// eye, so a model far from the origin does not jitter (PLAN 4).
//
// Depth is reversed: 1 at the near plane, 0 at the far plane, and the
// buffer is 32-bit float.
//
// Colours are sRGB values, blended as the feel test's canvas blends them.

struct Globals {
    view_proj: mat4x4<f32>,
    eye_hi: vec4<f32>,
    // xyz: the eye's low part; w: the near distance.
    eye_lo: vec4<f32>,
    // width px, height px, pixels per point, horizon y px
    viewport: vec4<f32>,
    // light direction xyz, ambient
    light: vec4<f32>,
    // diffuse, profile width (points), grid fade distance, grid push
    params: vec4<f32>,
    front: vec4<f32>,
    back: vec4<f32>,
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    ground_horizon: vec4<f32>,
    ground_bottom: vec4<f32>,
    // direction the light travels xyz, shadow alpha
    shadow: vec4<f32>,
    // tile px (points), dot px (points)
    stipple: vec4<f32>,
};

// Per draw: a colour and how to use it.
struct Draw {
    color: vec4<f32>,
    // mode (0 plain, 1 stipple; for lines: 0 normal, 1 hidden dashed, 2 hidden faint),
    // alpha factor, dash px, gap px
    mode: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var<uniform> d: Draw;
// Lines only: which face is nearest at each pixel, and its depth.
@group(2) @binding(0) var ids: texture_2d<u32>;
// Lines only: that face's plane, as bits: normal xyz and offset, relative
// to the eye.
@group(2) @binding(1) var planes: texture_2d<u32>;


fn rel(hi: vec3<f32>, lo: vec3<f32>) -> vec3<f32> {
    return (hi - g.eye_hi.xyz) + (lo - g.eye_lo.xyz);
}

// ---------- background: sky and ground gradients ----------

struct BgOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn bg_vs(@builtin(vertex_index) i: u32) -> BgOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: BgOut;
    o.pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return o;
}

fn ground_color(y: f32) -> vec4<f32> {
    let h = g.viewport.y;
    let hy = g.viewport.w;
    let y0 = max(0.0, hy);
    let t = clamp((y - y0) / max(1.0, h - y0), 0.0, 1.0);
    return mix(g.ground_horizon, g.ground_bottom, t);
}

fn sky_color(y: f32) -> vec4<f32> {
    let hy = g.viewport.w;
    let t = clamp(y / max(1.0, min(g.viewport.y, hy)), 0.0, 1.0);
    return mix(g.sky_top, g.sky_horizon, t);
}

@fragment
fn bg_fs(in: BgOut) -> @location(0) vec4<f32> {
    let y = in.pos.y;
    if (y < g.viewport.w) {
        return sky_color(y);
    }
    return ground_color(y);
}

// ---------- faces ----------

struct FaceIn {
    @location(0) hi: vec3<f32>,
    @location(1) lo: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) face: u32,
    @location(4) faded: u32,
};

struct FaceOut {
    // Invariant: the face-id pass must get exactly the same depths.
    @builtin(position) @invariant pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) @interpolate(flat) face: u32,
    @location(2) @interpolate(flat) faded: u32,
    // The point relative to the eye (for the face's plane in the id pass).
    @location(3) r: vec3<f32>,
};

@vertex
fn face_vs(v: FaceIn) -> FaceOut {
    var o: FaceOut;
    o.r = rel(v.hi, v.lo);
    o.pos = g.view_proj * vec4<f32>(o.r, 1.0);
    o.normal = v.normal;
    o.face = v.face;
    o.faded = v.faded;
    return o;
}

@fragment
fn face_fs(in: FaceOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = normalize(in.normal);
    var base = g.front;
    if (!front) {
        n = -n;
        base = g.back;
    }
    let sh = g.light.w + g.params.x * max(0.0, dot(n, g.light.xyz));
    var c = base.rgb * sh;
    // Outside the open group: washed out toward white, so the group stands
    // out (PLAN 1.2).
    if (in.faded != 0u) {
        c = mix(c, vec3<f32>(1.0, 1.0, 1.0), 0.6);
    }
    return vec4<f32>(c, 1.0);
}

// A second pass over the faces, with the depth test "equal", writes which
// face won each pixel, its depth and its plane. The lines test themselves
// against it.
struct IdOut {
    @location(0) id: vec2<u32>,
    @location(1) plane: vec4<u32>,
};

@fragment
fn id_fs(in: FaceOut) -> IdOut {
    var o: IdOut;
    o.id = vec2<u32>(in.face, bitcast<u32>(in.pos.z));
    let n = normalize(in.normal);
    o.plane = vec4<u32>(bitcast<vec3<u32>>(n), bitcast<u32>(dot(n, in.r)));
    return o;
}

// A tint over faces already drawn (hover, selection): plain or stipple.
@fragment
fn tint_fs(in: FaceOut) -> @location(0) vec4<f32> {
    if (d.mode.x > 0.5) {
        let p = in.pos.xy / g.viewport.z;
        let tile = g.stipple.x;
        let dot_size = g.stipple.y;
        let m = p - floor(p / tile) * tile;
        let a = all(m >= vec2<f32>(1.0)) && all(m < vec2<f32>(1.0 + dot_size));
        let b = all(m >= vec2<f32>(4.0)) && all(m < vec2<f32>(4.0 + dot_size));
        if (!(a || b)) {
            discard;
        }
    }
    return d.color;
}

// ---------- ground shadow ----------

@vertex
fn shadow_vs(v: FaceIn) -> FaceOut {
    var r = rel(v.hi, v.lo);
    let z = r.z + g.eye_hi.z + g.eye_lo.z;
    if (z > 0.0) {
        r = r + g.shadow.xyz * (-z / g.shadow.z);
    }
    var o: FaceOut;
    o.pos = g.view_proj * vec4<f32>(r, 1.0);
    o.normal = v.normal;
    o.r = r;
    return o;
}

@fragment
fn shadow_fs(in: FaceOut) -> @location(0) vec4<f32> {
    // Drawn with a "min" blend, so overlapping shadows darken only once.
    let c = ground_color(in.pos.y);
    return vec4<f32>(c.rgb * (1.0 - g.shadow.w), 1.0);
}

// ---------- lines (instanced screen-space quads) ----------

struct LineIn {
    @location(0) a_hi: vec3<f32>,
    @location(1) a_lo: vec3<f32>,
    @location(2) b_hi: vec3<f32>,
    @location(3) b_lo: vec3<f32>,
    @location(4) n1: vec3<f32>,
    @location(5) n2: vec3<f32>,
    @location(6) color: vec4<f32>,
    // width (points), dash (points), gap (points)
    @location(7) shape: vec3<f32>,
    @location(8) flags: u32,
    // The faces this edge belongs to (numbered from 1; 0 for none).
    @location(9) faces: vec2<u32>,
};

// Flags.
const HAS_N1: u32 = 1u;
const HAS_N2: u32 = 2u;
const MODEL_EDGE: u32 = 4u;
const FADE: u32 = 8u;
const PUSH_BACK: u32 = 16u;
// A vertex dot: shown unless a face is in front of the vertex itself.
const POINT: u32 = 64u;
// Tool feedback, drawn over everything.
const NO_TEST: u32 = 128u;
// A glyph: shape number in the dash slot, radius in the gap slot.
const MARK: u32 = 256u;
// A line may be this fraction of its depth behind a face and still show.
const DEPTH_SLACK: f32 = 5e-7;
// A line lies in a face's plane when both its ends are within this
// fraction of their distance from the eye (2 mm at 1 km; f32 rounding is
// about 100 times smaller).
const PLANE_SLACK: f32 = 2e-6;

struct LineOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    // along px, across px, length px, half width px
    @location(1) geo: vec4<f32>,
    // dash px, gap px
    @location(2) dash: vec2<f32>,
    @location(3) @interpolate(flat) faces: vec2<u32>,
    @location(4) @interpolate(flat) flags: u32,
    // Depth at each end.
    @location(5) @interpolate(flat) ends: vec2<f32>,
    // The line's direction on screen.
    @location(6) @interpolate(flat) dir: vec2<f32>,
    // Its ends relative to the eye.
    @location(7) @interpolate(flat) ra: vec3<f32>,
    @location(8) @interpolate(flat) rb: vec3<f32>,
};

fn to_px(c: vec4<f32>) -> vec2<f32> {
    let ndc = c.xy / c.w;
    return vec2<f32>((ndc.x * 0.5 + 0.5) * g.viewport.x, (0.5 - ndc.y * 0.5) * g.viewport.y);
}

@vertex
fn line_vs(@builtin(vertex_index) vi: u32, l: LineIn) -> LineOut {
    var o: LineOut;
    let ra = rel(l.a_hi, l.a_lo);
    let rb = rel(l.b_hi, l.b_lo);
    o.ra = ra;
    o.rb = rb;
    var ca = g.view_proj * vec4<f32>(ra, 1.0);
    var cb = g.view_proj * vec4<f32>(rb, 1.0);
    // Clip at the near plane (clip w is the distance along the view).
    let wmin = g.eye_lo.w;
    if (ca.w < wmin && cb.w < wmin) {
        o.pos = vec4<f32>(2.0, 2.0, 0.5, 1.0);
        return o;
    }
    if (ca.w < wmin) {
        ca = mix(ca, cb, (wmin - ca.w) / (cb.w - ca.w));
    }
    if (cb.w < wmin) {
        cb = mix(cb, ca, (wmin - cb.w) / (ca.w - cb.w));
    }
    var sa = to_px(ca);
    var sb = to_px(cb);
    var za = ca.z / ca.w;
    var zb = cb.z / cb.w;
    // Then to a band around the picture, so nothing gets huge. Depth is
    // affine along the line on screen, so it is cut the same way.
    let band = max(g.viewport.x, g.viewport.y);
    let lo = vec2<f32>(-band);
    let hi = vec2<f32>(g.viewport.x, g.viewport.y) + band;
    var t0 = 0.0;
    var t1 = 1.0;
    let dv = sb - sa;
    for (var i = 0; i < 2; i++) {
        if (abs(dv[i]) < 1e-9) {
            if (sa[i] < lo[i] || sa[i] > hi[i]) {
                t0 = 2.0;
            }
        } else {
            let ta = (lo[i] - sa[i]) / dv[i];
            let tb = (hi[i] - sa[i]) / dv[i];
            t0 = max(t0, min(ta, tb));
            t1 = min(t1, max(ta, tb));
        }
    }
    if (t0 > t1) {
        o.pos = vec4<f32>(2.0, 2.0, 0.5, 1.0);
        return o;
    }
    let a2 = sa + dv * t0;
    let b2 = sa + dv * t1;
    let za2 = mix(za, zb, t0);
    zb = mix(za, zb, t1);
    za = za2;
    sa = a2;
    sb = b2;
    let ppp = g.viewport.z;
    var width = l.shape.x;
    // Profile lines: edges between a face turned toward the eye and one
    // turned away, or edges with one face.
    if ((l.flags & MODEL_EDGE) != 0u && g.params.y > 0.0) {
        let to_eye = -(ra + rb) * 0.5;
        let h1 = (l.flags & HAS_N1) != 0u;
        let h2 = (l.flags & HAS_N2) != 0u;
        let s1 = dot(l.n1, to_eye) > 0.0;
        let s2 = dot(l.n2, to_eye) > 0.0;
        if ((h1 && !h2) || (h1 && h2 && s1 != s2)) {
            width = max(width, g.params.y);
        }
    }
    let half = width * ppp * 0.5;
    let reach = half + 1.0;
    var dir = sb - sa;
    let len = length(dir);
    if (len > 1e-6) {
        dir = dir / len;
    } else {
        dir = vec2<f32>(1.0, 0.0);
    }
    let nrm = vec2<f32>(-dir.y, dir.x);
    // Two triangles: corners (end, side).
    var ends = array<f32, 6>(0.0, 1.0, 0.0, 0.0, 1.0, 1.0);
    var sides = array<f32, 6>(-1.0, -1.0, 1.0, 1.0, -1.0, 1.0);
    let e = ends[vi];
    let s = sides[vi];
    let along = select(-reach, len + reach, e > 0.5);
    let across = s * reach;
    let p = sa + dir * along + nrm * across;
    let z = select(za, zb, e > 0.5);
    o.pos = vec4<f32>(p.x / g.viewport.x * 2.0 - 1.0, 1.0 - p.y / g.viewport.y * 2.0, z, 1.0);
    var color = l.color;
    if ((l.flags & FADE) != 0u) {
        let dist = length((ra + rb) * 0.5);
        color.a = color.a * clamp(1.25 - dist / g.params.z, 0.0, 1.0);
    }
    o.color = color;
    o.geo = vec4<f32>(along, across, len, half);
    o.dash = l.shape.yz * ppp;
    o.faces = l.faces;
    o.flags = l.flags;
    o.ends = vec2<f32>(za, zb);
    o.dir = dir;
    return o;
}

// The face number and depth stored at a pixel (clamped to the picture).
fn face_at(k: vec2<i32>) -> vec2<u32> {
    let size = vec2<i32>(textureDimensions(ids));
    return textureLoad(ids, clamp(k, vec2<i32>(0), size - 1), 0).xy;
}

// The depth of face `face` at screen point `p`, from the pixel `k` it is in
// and the face's depth gradient from neighbouring pixels of the same face
// (a planar face's depth is affine on screen, so this is exact).
fn face_depth(face: u32, d0: f32, k: vec2<i32>, p: vec2<f32>) -> f32 {
    var grad = vec2<f32>(0.0);
    let steps = array<vec2<i32>, 2>(vec2<i32>(1, 0), vec2<i32>(0, 1));
    for (var i = 0; i < 2; i++) {
        let s = steps[i];
        let a = face_at(k + s);
        let b = face_at(k - s);
        var gr = 0.0;
        if (a.x == face) {
            gr = bitcast<f32>(a.y) - d0;
        } else if (b.x == face) {
            gr = d0 - bitcast<f32>(b.y);
        }
        grad[i] = gr;
    }
    let c = vec2<f32>(k) + vec2<f32>(0.5);
    return d0 + dot(grad, p - c);
}

// Whether the line is in front of whatever face is drawn where it runs.
// Its own faces never hide it, so it needs no depth bias. Every fragment
// across the line's width asks about the same point on its centre line, so
// a line is hidden or shown across its whole width at once.
fn line_visible(in: LineOut) -> bool {
    if ((in.flags & NO_TEST) != 0u) {
        return true;
    }
    let along = in.geo.x;
    let across = in.geo.y;
    let len = in.geo.z;
    let nrm = vec2<f32>(-in.dir.y, in.dir.x);
    let tl = clamp(along, 0.0, len);
    let pc = in.pos.xy - nrm * across - in.dir * (along - tl);
    let k = vec2<i32>(floor(pc));
    let t = face_at(k);
    let face = t.x;
    if (face == 0u || face == in.faces.x || face == in.faces.y) {
        return true;
    }
    // A face never hides a line lying in its own plane, however many faces
    // share the line (owner feedback, 2026-11-09). Not the ground grid and
    // axes: they sit behind faces lying on the ground.
    let pl = textureLoad(planes, clamp(k, vec2<i32>(0), vec2<i32>(textureDimensions(planes)) - 1), 0);
    let n = bitcast<vec3<f32>>(pl.xyz);
    let off = bitcast<f32>(pl.w);
    if ((in.flags & PUSH_BACK) == 0u
        && abs(dot(n, in.ra) - off) <= PLANE_SLACK * length(in.ra)
        && abs(dot(n, in.rb) - off) <= PLANE_SLACK * length(in.rb)) {
        return true;
    }
    if ((in.flags & POINT) != 0u) {
        // Any face through the vertex has the vertex's depth there, exactly
        // up to rounding; only a face clearly in front hides it.
        return max(in.ends.x, in.ends.y) >= face_depth(face, bitcast<f32>(t.y), k, pc) * (1.0 - 64.0 * DEPTH_SLACK);
    }
    // The line's depth there: exact along it, and the nearer end for a line
    // seen end on.
    var z = max(in.ends.x, in.ends.y);
    if (len >= 1.0) {
        z = mix(in.ends.x, in.ends.y, tl / len);
    }
    if ((in.flags & PUSH_BACK) != 0u) {
        // The ground grid and axes sit just behind faces lying on the ground.
        z = z * g.params.w;
    }
    return z >= face_depth(face, bitcast<f32>(t.y), k, pc) * (1.0 - DEPTH_SLACK);
}

@fragment
fn line_fs(in: LineOut) -> @location(0) vec4<f32> {
    if ((in.flags & MARK) != 0u) {
        return mark(in);
    }
    // Mode 0 draws visible lines; modes 1 and 2 draw only hidden ones.
    if (line_visible(in) != (d.mode.x < 0.5)) {
        discard;
    }
    let along = in.geo.x;
    let across = in.geo.y;
    let len = in.geo.z;
    let half = in.geo.w;
    var dist = abs(across);
    if (along < 0.0) {
        dist = length(vec2<f32>(along, across));
    } else if (along > len) {
        dist = length(vec2<f32>(along - len, across));
    }
    let cover = clamp(half + 0.5 - dist, 0.0, 1.0);
    if (cover <= 0.0) {
        discard;
    }
    var dash = in.dash;
    if (d.mode.x > 0.5 && d.mode.x < 1.5) {
        dash = d.mode.zw * g.viewport.z;
    }
    if (dash.x > 0.0) {
        let period = dash.x + dash.y;
        let t = along - floor(along / period) * period;
        if (t > dash.x) {
            discard;
        }
    }
    var color = in.color;
    if (d.color.a > 0.0) {
        color = vec4<f32>(d.color.rgb, color.a);
    }
    return vec4<f32>(color.rgb, color.a * cover * d.mode.y);
}

// Glyphs (feel test `glyph(`): a coloured shape with a white outline.
// Shapes: 0 circle, 1 triangle, 2 square, 3 diamond, 4 cross, 5 ring,
// 6 diamond outline.
fn mark(in: LineOut) -> vec4<f32> {
    let ppp = g.viewport.z;
    let shape = u32(round(in.dash.x / ppp));
    let r = in.dash.y;
    // Screen offset from the centre (y down).
    let p = vec2<f32>(in.geo.x, in.geo.y);
    let edge = 2.0 * ppp;
    var d = length(p) - r;
    var stroke_only = false;
    var thin = 0.0;
    switch shape {
        case 1u: {
            // Pointing up: corners (0,-r-1), (r+1,r), (-r-1,r).
            let h = r + ppp;
            let a = vec2<f32>(abs(p.x), p.y);
            // The slanted side's outward normal.
            let n = normalize(vec2<f32>(r + h, -h));
            d = max(a.y - r, dot(a - vec2<f32>(0.0, -h), n));
        }
        case 2u: {
            d = max(abs(p.x), abs(p.y)) - r;
        }
        case 3u: {
            d = (abs(p.x) + abs(p.y) - (r + ppp)) * 0.7071;
        }
        case 4u: {
            // Two diagonal bars.
            let q = abs(p);
            let off = abs(q.x - q.y) * 0.7071;
            d = max(off - ppp, max(q.x, q.y) - r);
        }
        case 5u: {
            // A soft disc with a ring of the colour.
            let ring = abs(length(p) - r) - ppp;
            let ca = clamp(0.5 - ring, 0.0, 1.0);
            let fill = clamp(0.5 - (length(p) - r), 0.0, 1.0) * 0.2;
            let a = max(ca, fill);
            if (a <= 0.0) {
                discard;
            }
            return vec4<f32>(in.color.rgb, a * in.color.a);
        }
        case 6u: {
            stroke_only = true;
            d = (abs(p.x) + abs(p.y) - r) * 0.7071;
            thin = 0.75 * ppp;
        }
        default: {}
    }
    if (stroke_only) {
        let a = clamp(0.5 - (abs(d) - thin), 0.0, 1.0);
        if (a <= 0.0) {
            discard;
        }
        return vec4<f32>(in.color.rgb, a * in.color.a);
    }
    let inner = clamp(0.5 - d, 0.0, 1.0);
    let outer = clamp(0.5 - (d - edge), 0.0, 1.0);
    if (outer <= 0.0) {
        discard;
    }
    // White outline, colour inside.
    let c = mix(vec3<f32>(1.0), in.color.rgb, inner);
    return vec4<f32>(c, outer * in.color.a);
}

// ---------- tool feedback: filled triangles (the rectangle preview) ----------

struct TriIn {
    @location(0) hi: vec3<f32>,
    @location(1) lo: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct TriOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn tri_vs(v: TriIn) -> TriOut {
    var o: TriOut;
    o.pos = g.view_proj * vec4<f32>(rel(v.hi, v.lo), 1.0);
    o.color = v.color;
    return o;
}

@fragment
fn tri_fs(in: TriOut) -> @location(0) vec4<f32> {
    return in.color;
}
