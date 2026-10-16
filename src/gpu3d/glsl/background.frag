#version 330 core
in float vY;
out vec4 oColor;
void main() {
    float t = clamp(vY, 0.0, 1.0);
    float g = 0.12 + 0.08 * (1.0 - t);
    oColor = vec4(vec3(g, g, g * 1.05), 1.0);
}
