#version 100
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif

varying vec2 uv;

uniform sampler2D Texture; // the scene
uniform sampler2D atlas;   // 16 x 6 glyphs, 12 x 20 px each, coverage in alpha
uniform sampler2D layer;   // explicit characters: rgb = colour, a = char code
uniform sampler2D ramp;    // characters sorted by ink: a = atlas slot

uniform vec2 resolution;
uniform vec2 cell;
uniform vec2 origin;
uniform vec2 grid;
uniform float time;
uniform float flash;
uniform float mode;
uniform float ramp_len;
uniform float flip;

const vec2 GLYPH = vec2(12.0, 20.0);
const vec2 ATLAS = vec2(192.0, 120.0);

float hash(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.5;
    for (int k = 0; k < 4; k++) {
        v += a * noise(p);
        p = p * 2.03 + vec2(17.1, 9.2);
        a *= 0.5;
    }
    return v;
}

vec4 scene_at(vec2 px) {
    vec2 u = px / resolution;
    if (flip > 0.5) u.y = 1.0 - u.y;
    return texture2D(Texture, u);
}

// Coverage of atlas glyph `idx` at `local` (0..1 inside the cell),
// supersampled so small cells stay smooth.
float glyph(float idx, vec2 local) {
    vec2 slot = vec2(mod(idx, 16.0), floor(idx / 16.0 + 0.001)) * GLYPH;
    vec2 d = 0.25 * GLYPH / cell;
    vec2 p = local * GLYPH;
    float m = texture2D(atlas, (slot + clamp(p + vec2(-d.x, -d.y), vec2(0.5), GLYPH - 0.5)) / ATLAS).a;
    m += texture2D(atlas, (slot + clamp(p + vec2( d.x, -d.y), vec2(0.5), GLYPH - 0.5)) / ATLAS).a;
    m += texture2D(atlas, (slot + clamp(p + vec2(-d.x,  d.y), vec2(0.5), GLYPH - 0.5)) / ATLAS).a;
    m += texture2D(atlas, (slot + clamp(p + vec2( d.x,  d.y), vec2(0.5), GLYPH - 0.5)) / ATLAS).a;
    return m * 0.25;
}

void main() {
    vec2 px = uv * resolution;
    if (mode > 1.5) {
        gl_FragColor = vec4(scene_at(px).rgb, 1.0);
        return;
    }

    vec2 g = (px - origin) / cell;
    vec2 id = floor(g);
    vec2 local = g - id;
    vec2 c = (id + 0.5) * cell + origin;

    // average the scene over this cell
    vec4 s = 0.25 * (scene_at(c + cell * vec2(-0.25, -0.25)) + scene_at(c + cell * vec2(0.25, -0.25))
                   + scene_at(c + cell * vec2(-0.25, 0.25)) + scene_at(c + cell * vec2(0.25, 0.25)));
    float lum = dot(s.rgb, vec3(0.299, 0.587, 0.114));
    vec3 col = s.rgb;
    float L = lum;

    // drifting background field wherever nothing opaque was drawn
    vec2 bp = id * vec2(0.07, 0.12);
    float n = fbm(bp + vec2(time * 0.03, -time * 0.09));
    float tone = fbm(bp * 0.35 - vec2(time * 0.01, 0.0));
    float tw = step(0.994, hash(id + floor(time * 1.5 + hash(id * 1.7) * 9.0)));
    // anything drawn with real opacity (panels, the well) hides the field
    float open = 1.0 - clamp(s.a * 2.5, 0.0, 1.0);
    float bgl = max(smoothstep(0.42, 0.9, n) * 0.3, tw * 0.45) * open;
    // scene content gets a steeper curve so blocks become dense characters
    float level = pow(L, 0.5);
    if (bgl > L) {
        L = bgl;
        level = pow(L, 0.9);
        col = mix(mix(vec3(0.20, 0.46, 0.28), vec3(0.30, 0.40, 0.52), tone), vec3(0.85, 0.95, 0.9), tw);
    }

    // brightness -> character with that much ink (with a little variety)
    float r = level * (ramp_len - 1.0) + (hash(id + 3.1) - 0.5) * 5.0;
    r = clamp(floor(r + 0.5), 0.0, ramp_len - 1.0);
    float idx = floor(texture2D(ramp, vec2((r + 0.5) / ramp_len, 0.5)).a * 255.0 + 0.5);
    float on = step(0.03, L);
    vec3 hue = col / max(max(col.r, max(col.g, col.b)), 0.001);
    col = hue * mix(0.3, 1.15, sqrt(L));

    // explicit characters win
    if (id.x >= 0.0 && id.y >= 0.0 && id.x < grid.x && id.y < grid.y) {
        vec4 l = texture2D(layer, (id + 0.5) / grid);
        if (l.a > 0.0) {
            idx = floor(l.a * 255.0 + 0.5) - 32.0;
            col = l.rgb;
            on = 1.0;
        }
    }

    vec3 outc = col * glyph(idx, local) * on;

    if (mode < 0.5) {
        // soft glow from bright parts of the scene
        vec3 halo = scene_at(c + cell * vec2(2.0, 0.0)).rgb + scene_at(c - cell * vec2(2.0, 0.0)).rgb
                  + scene_at(c + cell * vec2(0.0, 1.2)).rgb + scene_at(c - cell * vec2(0.0, 1.2)).rgb
                  + scene_at(c + cell * vec2(3.5, 2.0)).rgb + scene_at(c - cell * vec2(3.5, 2.0)).rgb
                  + scene_at(c + cell * vec2(-3.5, 2.0)).rgb + scene_at(c - cell * vec2(-3.5, 2.0)).rgb;
        outc += halo * 0.022;
        outc *= 0.93 + 0.07 * sin(px.y * 3.14159);            // scanlines
        vec2 q = uv - 0.5;
        outc *= 1.0 - dot(q, q) * 0.85;                        // vignette
        outc += (hash(px + fract(time * 7.0) * 91.0) - 0.5) * 0.03; // grain
        outc += flash * vec3(0.45, 0.6, 0.6) * 0.4;
    }
    gl_FragColor = vec4(outc, 1.0);
}