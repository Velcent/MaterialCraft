// The backdrop: the grey gradient, with the environment over it at the chosen opacity and blur (EN-05). Drawn
// apart from the scene, so it never changes the lighting.
in vec2 vNdc;
uniform mat4 uInvVP;      // clip -> world of the plain camera
uniform float uOpacity;   // 0: gradient only
uniform float uBlur;      // 0 sharp … 1 the roughest level
uniform float uExposure;  // the environment's exposure times the camera's (linear)
uniform int uStudioKey;   // the studio: add its key softbox
uniform vec3 uKey;
out vec4 oColor;

void main() {
    float t = clamp(vNdc.y * 0.5 + 0.5, 0.0, 1.0);
    float g = 0.12 + 0.08 * (1.0 - t);
    vec3 col = vec3(g, g, g * 1.05);
    if (uOpacity > 0.0) {
        vec4 a = uInvVP * vec4(vNdc, -1.0, 1.0);
        vec4 b = uInvVP * vec4(vNdc, 1.0, 1.0);
        vec3 ray = normalize(b.xyz / b.w - a.xyz / a.w);
        float r = sqrt(clamp(uBlur, 0.0, 1.0));
        vec3 env = envSpecular(ray, r);
        if (uStudioKey == 1) env += studioKey(ray, r, uKey);
        vec3 shown = display(curve(max(env, vec3(0.0)) * uExposure));
        col = mix(col, shown, clamp(uOpacity, 0.0, 1.0));
    }
    oColor = vec4(col, 1.0);
}
