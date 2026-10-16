#version 330 core
in vec3 vPos;
in vec3 vN;
in vec3 vT;
in vec2 vUv;
in vec2 vScale;
flat in float vTanW;
uniform sampler2D uBase;
uniform sampler2D uNormal;
uniform sampler2D uRma;
uniform sampler2D uEmis;
uniform sampler2D uHeight;
uniform vec3 uEye;
uniform vec3 uL;
uniform vec3 uFill;
uniform float uLightI;
uniform float uExposure;
uniform float uAmbient;
uniform float uNormalAmt;
uniform float uBump;
uniform float uDepth;
uniform float uLevel;
uniform float uTexel;
uniform int uMode;
uniform int uHasHeight;
uniform int uHasEmis;
uniform int uWire;
uniform int uShadow;
uniform int uCull;
out vec4 oColor;

const float PI = 3.14159265;
vec2 gdx;
vec2 gdy;

// Smooth gradient only: used for diffuse ambient light.
vec3 envSoft(vec3 d) {
    vec3 sky = vec3(0.30, 0.34, 0.42);
    vec3 hor = vec3(0.52, 0.52, 0.53);
    vec3 ground = vec3(0.20, 0.19, 0.18);
    return d.y > 0.0 ? mix(hor, sky, pow(d.y, 0.6)) : mix(hor, ground, pow(-d.y, 0.5));
}

// Studio environment for reflections: softboxes, a rim strip and a horizon line on a dark floor. The boxes soften and
// dim with roughness, so a smooth surface mirrors clear shapes and a rough one only shows a broad sheen.
vec3 env(vec3 d, float r) {
    float soft = r * r;
    vec3 col = mix(envSoft(d), vec3(0.30), soft * 0.5);
    float w = 0.015 + soft * 0.9;
    float keyI = 4.5 / (1.0 + soft * 14.0);
    float key = smoothstep(0.93 - w, 0.93 + 0.015, dot(d, uL));
    col += vec3(1.0, 0.96, 0.90) * key * keyI;
    // tall side strip opposite the key
    vec3 side = normalize(vec3(-uL.z, 0.0, uL.x));
    float sdot = dot(normalize(vec3(d.x, 0.0, d.z) + 1e-5), side);
    float strip = smoothstep(0.93 - w, 0.97, sdot) * smoothstep(0.02 - w, 0.12, d.y) * (1.0 - smoothstep(0.55, 0.75 + w, d.y));
    col += vec3(0.85, 0.92, 1.0) * strip * 1.5 / (1.0 + soft * 10.0);
    // top panel
    float top = smoothstep(0.90 - w, 0.97, d.y);
    col += vec3(1.0) * top * 1.0 / (1.0 + soft * 10.0);
    // bright horizon line
    float hl = exp(-pow(d.y / (0.035 + soft * 0.4), 2.0));
    col += vec3(0.9) * hl * 0.25 / (1.0 + soft * 8.0);
    return col;
}

vec3 aces(vec3 x) {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}

// Tone map on luminance so saturated colours keep their hue (a per-channel curve turns bright oranges pink and white),
// then let only the very brightest highlights roll off per channel.
float acesL(float x) {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}
vec3 tonemap(vec3 c) {
    float l = dot(c, vec3(0.2126, 0.7152, 0.0722));
    vec3 hue = c * (acesL(l) / max(l, 1e-4));
    hue = min(hue, vec3(1.0)) ;
    return mix(hue, aces(c), smoothstep(2.0, 8.0, l) * 0.7);
}

vec3 envBrdf(vec3 f0, float rough, float ndv) {
    const vec4 c0 = vec4(-1.0, -0.0275, -0.572, 0.022);
    const vec4 c1 = vec4(1.0, 0.0425, 1.04, -0.04);
    vec4 r = rough * c0 + c1;
    float a004 = min(r.x * r.x, exp2(-9.28 * ndv)) * r.x + r.y;
    vec2 ab = vec2(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + ab.y;
}

float hAt(vec2 uv) {
    return textureGrad(uHeight, uv, gdx, gdy).r;
}

// Outward relief mapping: the height field is a slab of thickness uDepth sitting on the surface; uLevel is flush with
// the polygon and brighter texels rise out of it. Returns the shifted uv and how deep (0..1) the hit lies below the top.
vec2 pom(vec2 uv, vec3 vt, out float cav) {
    float depth = uDepth;
    float vz = max(vt.z, 0.2);
    vec2 d = vec2(-vt.x / vz / vScale.x, vt.y / vz / vScale.y);
    float top = (1.0 - clamp(uLevel, 0.0, 1.0)) * depth;
    cav = 0.0;
    float f0 = (1.0 - hAt(uv + d * (0.0 - top))) * depth;
    if (f0 <= 0.0) return uv + d * (0.0 - top);
    const int STEPS = 64;
    float prevD = 0.0;
    for (int i = 1; i <= STEPS; i++) {
        float dd = depth * float(i) / float(STEPS);
        float f = (1.0 - hAt(uv + d * (dd - top))) * depth - dd;
        if (f <= 0.0) {
            float lo = prevD;
            float hi = dd;
            for (int k = 0; k < 6; k++) {
                float mid = 0.5 * (lo + hi);
                float fm = (1.0 - hAt(uv + d * (mid - top))) * depth - mid;
                if (fm > 0.0) lo = mid; else hi = mid;
            }
            float dh = 0.5 * (lo + hi);
            cav = clamp(dh / depth, 0.0, 1.0);
            return uv + d * (dh - top);
        }
        prevD = dd;
    }
    cav = 1.0;
    return uv + d * (depth - top);
}

// Contact shadow: march from the shaded point towards the light through the height field.
float hshadow(vec2 uv, vec3 lt) {
    if (lt.z <= 0.02) return 1.0;
    float depth = uDepth;
    float h0 = (hAt(uv) - uLevel) * depth + 0.01 * depth;
    float slope = lt.z / max(length(lt.xy), 1e-3);
    vec2 hd = lt.xy / max(length(lt.xy), 1e-4);
    float maxS = ((1.0 - uLevel) * depth - h0) / slope;
    if (maxS <= 0.0) return 1.0;
    float res = 1.0;
    const int N = 32;
    // interleaved gradient noise: turns the stepping bands of the march into fine noise
    float jit = fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715))));
    for (int i = 1; i <= N; i++) {
        float s = maxS * (float(i) - 1.0 + jit) / float(N);
        vec2 u2 = uv + vec2(hd.x / vScale.x, -hd.y / vScale.y) * s;
        float hs = (hAt(u2) - uLevel) * depth;
        float ray = h0 + s * slope;
        res = min(res, 1.0 - smoothstep(0.0, 0.10 * depth + 1e-5, hs - ray));
    }
    return mix(0.55, 1.0, res);
}

void main() {
    if (uWire == 1) {
        oColor = vec4(1.0, 0.62, 0.12, 1.0);
        return;
    }
    vec3 N = normalize(vN);
    // shapes seen from the inside: walls that face away from the camera are not drawn
    if (uCull == 1 && dot(N, uEye - vPos) < 0.0) discard;
    vec3 T = normalize(vT - N * dot(vT, N));
    vec3 B = cross(N, T) * vTanW; // uv v runs opposite to the bitangent
    vec3 V = normalize(uEye - vPos);
    bool back = !gl_FrontFacing;
    gdx = dFdx(vUv);
    gdy = dFdy(vUv);
    vec2 uv = vUv;
    float cav = 1.0;
    bool hasH = uHasHeight == 1 && uDepth > 0.0;
    if (uMode == 2 && hasH) {
        vec3 vt = vec3(dot(V, T), dot(V, B), dot(V, N) * (back ? -1.0 : 1.0));
        float dep;
        uv = pom(uv, vt, dep);
        cav = 1.0 - 0.45 * dep;
    }
    vec3 albedo = textureGrad(uBase, uv, gdx, gdy).rgb; // sRGB texture: already linear here
    vec3 rma = textureGrad(uRma, uv, gdx, gdy).rgb;
    float rough = clamp(rma.r, 0.04, 1.0);
    float metal = clamp(rma.g, 0.0, 1.0);
    float ao = mix(1.0, clamp(rma.b, 0.0, 1.0), 0.85) * cav;
    vec3 nm = textureGrad(uNormal, uv, gdx, gdy).xyz;
    vec3 tn = vec3((nm.xy * 2.0 - 1.0) * uNormalAmt, max(nm.z * 2.0 - 1.0, 0.05));
    if (uMode == 1 && uHasHeight == 1 && uBump > 0.0) {
        float e = uTexel;
        float dhu = (hAt(uv + vec2(e, 0.0)) - hAt(uv - vec2(e, 0.0))) / (2.0 * e);
        float dhv = (hAt(uv + vec2(0.0, e)) - hAt(uv - vec2(0.0, e))) / (2.0 * e);
        float k = uBump * 0.12;
        tn.x -= k * dhu / vScale.x;
        tn.y += k * dhv / vScale.y;
    }
    tn = normalize(tn);
    vec3 n = normalize(T * tn.x + B * tn.y + N * tn.z);
    if (back) n = -n;

    float ndv = max(dot(n, V), 1e-3);
    vec3 f0 = mix(vec3(0.04), albedo, metal);
    float a = rough * rough;
    vec3 hv = normalize(uL + V);
    float ndl = max(dot(n, uL), 0.0);
    float ndh = max(dot(n, hv), 0.0);
    float vdh = max(dot(V, hv), 0.0);
    float dd = a * a / (PI * pow(ndh * ndh * (a * a - 1.0) + 1.0, 2.0) + 1e-6);
    float kk = (rough + 1.0) * (rough + 1.0) / 8.0;
    float g = (ndl / (ndl * (1.0 - kk) + kk)) * (ndv / (ndv * (1.0 - kk) + kk));
    float fr = pow(1.0 - vdh, 5.0);
    float sh = 1.0;
    if (uShadow == 1 && hasH && (uMode == 2 || uMode == 3) && !back) {
        vec3 lt = vec3(dot(uL, T), dot(uL, B), dot(uL, N));
        sh = hshadow(uv, lt);
    }
    vec3 f = f0 + (1.0 - f0) * fr;
    vec3 spec = dd * g * f / (4.0 * max(ndl, 1e-3) * ndv) * ndl;
    vec3 diff = albedo * (1.0 - metal) * (1.0 - f) / PI * ndl * uLightI * 1.15;
    float ndl2 = max(dot(n, uFill), 0.0);
    vec3 diffFill = albedo * (1.0 - metal) / PI * ndl2 * uLightI * 0.5;
    vec3 refl = reflect(-V, n);
    vec3 envR = env(refl, rough);
    vec3 envD = envSoft(n);
    vec3 ambD = albedo * (1.0 - metal) * envD * uAmbient;
    float specOcc = clamp(pow(ndv + ao, exp2(-16.0 * rough - 1.0)) - 1.0 + ao, 0.0, 1.0);
    float horizon = clamp(1.0 + 1.2 * dot(refl, back ? -N : N), 0.0, 1.0);
    vec3 ambS = envR * envBrdf(f0, rough, ndv) * uAmbient * specOcc * horizon * horizon;
    vec3 col = ((diff + spec * uLightI * 1.2) * sh + diffFill + (ambD * ao + ambS)) * uExposure;
    if (uHasEmis == 1) col += textureGrad(uEmis, uv, gdx, gdy).rgb;
    col = tonemap(col);
    col = pow(col, vec3(1.0 / 2.2));
    oColor = vec4(col, 1.0);
}
