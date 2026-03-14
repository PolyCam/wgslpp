#include "common.wgsl"

#ifdef USE_COLOR
fn get_color() -> vec3<f32> {
    return BRAND_COLOR;
}
#else
fn get_color() -> vec3<f32> {
    return vec3<f32>(1.0, 1.0, 1.0);
}
#endif

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(get_color(), 1.0);
}
