// Bakes the built-in studio (without its key softbox, see studio.glsl) into one environment level or the diffuse
// image: each texel holds the studio in that texel's direction.
uniform vec2 uSize;     // the target's size in texels
uniform float uRough;   // the level's roughness
uniform int uDiffuse;   // 1: the diffuse image (the studio's soft gradient)
out vec4 oColor;

void main() {
    vec2 uv = gl_FragCoord.xy / uSize;
    float phi = (uv.x - 0.5) * 2.0 * 3.14159265;
    float theta = uv.y * 3.14159265;
    vec3 d = vec3(sin(theta) * sin(phi), cos(theta), -sin(theta) * cos(phi));
    vec3 c = uDiffuse == 1 ? studioSoft(d) : studioRest(d, uRough);
    oColor = vec4(c, 1.0);
}
