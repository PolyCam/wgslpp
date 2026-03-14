#ifndef COMMON_WGSL
#define COMMON_WGSL

#define BRAND_COLOR vec3<f32>(0.2, 0.4, 0.8)

const PI: f32 = 3.14159265;

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

#endif
