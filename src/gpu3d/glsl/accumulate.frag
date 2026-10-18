#version 330 core
// Copies a frame into the accumulation target; the blend state weighs it into the running average.
uniform sampler2D uSrc;
out vec4 oColor;
void main() {
    oColor = texelFetch(uSrc, ivec2(gl_FragCoord.xy), 0);
}
