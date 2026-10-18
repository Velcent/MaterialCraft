// The tone stage shared by the post stage and the background: the tone curve and the display transform.
uniform int uCurve;       // 0 filmic, 1 linear, 2 ACES fit

// The ACES filmic curve as fitted by Krzysztof Narkowicz (2016).
vec3 acesFit(vec3 x) {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}
float acesFitL(float x) {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}

// The viewport's filmic curve: the fit on luminance so saturated colours keep their hue (a per-channel curve turns
// bright oranges pink and white), then only the very brightest highlights roll off per channel.
vec3 filmic(vec3 c) {
    float l = dot(c, vec3(0.2126, 0.7152, 0.0722));
    vec3 hue = c * (acesFitL(l) / max(l, 1e-4));
    hue = min(hue, vec3(1.0));
    return mix(hue, acesFit(c), smoothstep(2.0, 8.0, l) * 0.7);
}

vec3 curve(vec3 c) {
    if (uCurve == 1) return clamp(c, 0.0, 1.0);
    if (uCurve == 2) return acesFit(c);
    return filmic(c);
}

// The display transform: a 2.2 gamma encode (the hook for a colour-managed transform, a 3D LUT, later).
vec3 display(vec3 c) {
    return pow(c, vec3(1.0 / 2.2));
}
