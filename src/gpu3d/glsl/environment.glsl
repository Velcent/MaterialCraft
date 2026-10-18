// Environment lookups (shared by the material and the background): the sharp image (mirror reflections, the
// backdrop), the prefiltered levels (level i holds roughness i / (levels - 1); level 0 is taken from the sharp image),
// the diffuse image (E / pi) and the environment's turn about +Y. Lat-long layout: row 0 looks up,
// the centre looks along -Z, u grows towards +X.
uniform sampler2D uEnvSharp;
uniform sampler2DArray uEnvSpec;
uniform sampler2D uEnvDiff;
uniform float uEnvLevels;
uniform vec2 uEnvRot; // cos, sin of the environment's turn

const float ENV_PI = 3.14159265;

// World direction -> the environment's own frame.
vec3 envFrame(vec3 d) {
    return vec3(d.x * uEnvRot.x - d.z * uEnvRot.y, d.y, d.x * uEnvRot.y + d.z * uEnvRot.x);
}

vec2 envUv(vec3 d) {
    d = normalize(d);
    return vec2(atan(d.x, -d.z) / (2.0 * ENV_PI) + 0.5, acos(clamp(d.y, -1.0, 1.0)) / ENV_PI);
}

// Prefiltered radiance around world direction d for GGX roughness r (0..1), between the two nearest levels.
vec3 envSpecular(vec3 d, float r) {
    vec2 uv = envUv(envFrame(d));
    float f = clamp(r, 0.0, 1.0) * (uEnvLevels - 1.0);
    float i0 = floor(f);
    vec3 a = i0 < 0.5 ? textureLod(uEnvSharp, uv, 0.0).rgb : texture(uEnvSpec, vec3(uv, i0)).rgb;
    vec3 b = texture(uEnvSpec, vec3(uv, min(i0 + 1.0, uEnvLevels - 1.0))).rgb;
    return mix(a, b, f - i0);
}

// What a white diffuse surface with world normal n reflects.
vec3 envDiffuse(vec3 n) {
    return texture(uEnvDiff, envUv(envFrame(n))).rgb;
}
