#version 330 core
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec3 aNrm;
layout(location = 2) in vec3 aTan;
layout(location = 3) in vec2 aUv;
layout(location = 4) in vec3 aMisc; // su, sv, displacement fade
layout(location = 5) in float aTanW; // bitangent sign: -1 on mirrored uv islands
uniform mat4 uVP;
uniform sampler2D uHeight;
uniform float uDisp;
uniform float uDispLevel; // the height that stays on the surface
uniform float uVtxLod;
out vec3 vPos;
out vec3 vN;
out vec3 vT;
out vec2 vUv;
out vec2 vScale;
flat out float vTanW;
void main() {
    vec3 p = aPos;
    if (uDisp != 0.0) {
        float h = textureLod(uHeight, aUv, uVtxLod).r;
        p += aNrm * (h - uDispLevel) * uDisp * aMisc.z;
    }
    vPos = p;
    vN = aNrm;
    vT = aTan;
    vUv = aUv;
    vScale = aMisc.xy;
    vTanW = aTanW;
    gl_Position = uVP * vec4(p, 1.0);
}
