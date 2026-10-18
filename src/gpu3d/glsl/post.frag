// The post stage: the HDR frame (linear radiance times coverage, coverage in alpha) is exposed, tone mapped and
// display-encoded, then blended over the background as premultiplied colour.
uniform sampler2D uSrc;
uniform vec2 uOrigin;     // the viewport's lower-left corner in window pixels
uniform float uExposure;  // linear scale (2^stops)
uniform int uBypass;      // 1: the frame holds display values already (unlit shading)
out vec4 oColor;

void main() {
    vec4 s = texelFetch(uSrc, ivec2(gl_FragCoord.xy - uOrigin), 0);
    if (s.a <= 0.0) {
        oColor = vec4(0.0);
        return;
    }
    vec3 c = s.rgb / s.a;
    if (uBypass == 0) c = display(curve(max(c, vec3(0.0)) * uExposure));
    oColor = vec4(c * s.a, s.a);
}
