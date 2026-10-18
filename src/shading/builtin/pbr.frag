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
uniform int uUnlit;
uniform float uMipBias;
// wireframe overlay: drawn after the post stage, tested against the scene's depth by hand
uniform sampler2D uSceneDepth;
uniform vec2 uDepthOrigin;
uniform vec4 uDepthLin; // projection elements 10, 11, 14, 15
// declared parameters (pbr.json); their defaults reproduce the program as it was before shader files
uniform vec3 uBaseTint;
uniform float uSpecularLevel; // dielectric F0 = 0.08 * level (0.5: 4 %)
uniform float uAoStrength;
uniform float uEmissiveIntensity;
uniform int uSss;
uniform vec3 uSssColor;
uniform float uSssRadius;
uniform float uSssTranslucency;
// environment: its exposure as a linear scale, and whether the studio's key softbox is added to reflections
uniform float uEnvScale;
uniform int uStudioKey;
// scene shadows (EN-07): 0 off, 1 on the key light, 2 on the environment's dominant light share
uniform int uShadowMode;
uniform sampler2DShadow uShadowMap;
uniform mat4 uShadowVP;
uniform float uShadowOpacity;
uniform float uShadowOffset; // normal offset in world units (about two shadow-map texels)
uniform int uDomCount;
uniform vec3 uDomDir[4];
uniform vec3 uDomE[4];
out vec4 oColor;

const float PI = 3.14159265;
vec2 gdx;
vec2 gdy;

vec3 envBrdf(vec3 f0, float rough, float ndv) {
    const vec4 c0 = vec4(-1.0, -0.0275, -0.572, 0.022);
    const vec4 c1 = vec4(1.0, 0.0425, 1.04, -0.04);
    vec4 r = rough * c0 + c1;
    float a004 = min(r.x * r.x, exp2(-9.28 * ndv)) * r.x + r.y;
    vec2 ab = vec2(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + ab.y;
}

vec3 srgbEncode(vec3 c) {
    return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}

// Distance in front of the camera of a window depth (0 … 1), for perspective and parallel projections.
float viewDepth(float d) {
    float ndc = 2.0 * d - 1.0;
    return -(ndc * uDepthLin.w - uDepthLin.z) / (uDepthLin.x - ndc * uDepthLin.y);
}

// Subsurface scattering, approximated: light wraps past the terminator by the scatter radius (tinted by the
// scatter colour, as light that travelled under the surface), and some passes through thin parts towards the viewer
// when the light is behind the surface. Diffuse light only, added to the lit diffuse term.
vec3 subsurface(vec3 diffuseColor, vec3 n, vec3 V) {
    float w = uSssRadius;
    float ndl = dot(n, uL);
    float wrapped = max((ndl + w) / (1.0 + w), 0.0);
    vec3 scatter = uSssColor * max(wrapped - max(ndl, 0.0), 0.0);
    float back = pow(clamp(dot(V, -uL), 0.0, 1.0), 4.0) * uSssTranslucency;
    return diffuseColor * (scatter + uSssColor * back) / PI;
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

// Light reaching the point from the shadow map's direction: 1 lit, 0 shadowed (2 × 2 filtered by the hardware, plus
// four taps around it to soften the edge a little).
float shadowVis(vec3 gN) {
    vec3 p = vPos + gN * uShadowOffset;
    vec4 c = uShadowVP * vec4(p, 1.0);
    vec3 s = c.xyz / c.w * 0.5 + 0.5;
    if (s.x <= 0.0 || s.y <= 0.0 || s.x >= 1.0 || s.y >= 1.0 || s.z >= 1.0) return 1.0;
    vec2 texel = 1.0 / vec2(textureSize(uShadowMap, 0));
    float v = texture(uShadowMap, vec3(s.xy, s.z)) * 0.5;
    v += texture(uShadowMap, vec3(s.xy + vec2(1.0, 0.5) * texel, s.z)) * 0.125;
    v += texture(uShadowMap, vec3(s.xy + vec2(-1.0, -0.5) * texel, s.z)) * 0.125;
    v += texture(uShadowMap, vec3(s.xy + vec2(-0.5, 1.0) * texel, s.z)) * 0.125;
    v += texture(uShadowMap, vec3(s.xy + vec2(0.5, -1.0) * texel, s.z)) * 0.125;
    return mix(1.0, v, uShadowOpacity);
}

// The share of the environment's diffuse light at normal n that comes from its dominant lights (what a shadow
// from them takes away).
float dominantShare(vec3 n) {
    vec3 e = vec3(0.0);
    for (int i = 0; i < 4; i++) {
        if (i >= uDomCount) break;
        e += uDomE[i] * max(dot(n, uDomDir[i]), 0.0);
    }
    float total = dot(envDiffuse(n), vec3(0.2126, 0.7152, 0.0722)) * PI;
    return clamp(dot(e, vec3(0.2126, 0.7152, 0.0722)) / max(total, 1e-6), 0.0, 1.0);
}

void main() {
    if (uWire == 1) {
        float scene = texelFetch(uSceneDepth, ivec2(gl_FragCoord.xy - uDepthOrigin), 0).r;
        if (viewDepth(gl_FragCoord.z) > viewDepth(scene) * 1.003 + 0.002) discard;
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
    gdx = dFdx(vUv) * exp2(uMipBias);
    gdy = dFdy(vUv) * exp2(uMipBias);
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
    if (uUnlit == 1) {
        // the maps as they are: base colour plus emissive, encoded back to the display values they came from
        vec3 shown = albedo;
        if (uHasEmis == 1) shown += textureGrad(uEmis, uv, gdx, gdy).rgb;
        oColor = vec4(srgbEncode(clamp(shown, 0.0, 1.0)), 1.0);
        return;
    }
    albedo *= uBaseTint;
    vec3 rma = textureGrad(uRma, uv, gdx, gdy).rgb;
    float rough = clamp(rma.r, 0.04, 1.0);
    float metal = clamp(rma.g, 0.0, 1.0);
    float ao = mix(1.0, clamp(rma.b, 0.0, 1.0), uAoStrength) * cav;
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
    vec3 f0 = mix(vec3(0.08 * uSpecularLevel), albedo, metal);
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
    if (uSss == 1) diff += subsurface(albedo * (1.0 - metal), n, V) * uLightI * 1.15;
    float ndl2 = max(dot(n, uFill), 0.0);
    vec3 diffFill = albedo * (1.0 - metal) / PI * ndl2 * uLightI * 0.5;
    vec3 refl = reflect(-V, n);
    vec3 envR = envSpecular(refl, rough);
    if (uStudioKey == 1) envR += studioKey(refl, rough, uL);
    vec3 envD = envDiffuse(n);
    vec3 ambD = albedo * (1.0 - metal) * envD * uAmbient;
    float specOcc = clamp(pow(ndv + ao, exp2(-16.0 * rough - 1.0)) - 1.0 + ao, 0.0, 1.0);
    float horizon = clamp(1.0 + 1.2 * dot(refl, back ? -N : N), 0.0, 1.0);
    vec3 ambS = envR * envBrdf(f0, rough, ndv) * uAmbient * specOcc * horizon * horizon;
    float keyVis = 1.0;
    float envVis = 1.0;
    if (uShadowMode != 0 && !back) {
        float vis = shadowVis(N);
        if (uShadowMode == 1) keyVis = vis;
        else envVis = 1.0 - dominantShare(n) * (1.0 - vis);
    }
    vec3 col = ((diff + spec * uLightI * 1.2) * sh * keyVis + diffFill + (ambD * ao + ambS) * envVis) * uExposure * uEnvScale;
    if (uHasEmis == 1) col += textureGrad(uEmis, uv, gdx, gdy).rgb * uEmissiveIntensity;
    // linear radiance: exposure in stops, the tone curve and the display transform follow in the post stage
    oColor = vec4(col, 1.0);
}
