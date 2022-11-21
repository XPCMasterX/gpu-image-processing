pub mod invert {
    vulkano_shaders::shader! {
        ty: "compute",
        src: "
                #version 450
                layout(local_size_x = 4, local_size_y = 4, local_size_z = 1) in;
                
                layout(set = 0, binding = 0, rgba8) uniform image2D img;
                
                void main() {
                    vec4 data = imageLoad(img, ivec2(gl_GlobalInvocationID.xy));
                    vec4 to_write = vec4(1.0 - data[0], 1.0-data[1], 1.0 - data[2], 1.0);
                    
                    imageStore(img, ivec2(gl_GlobalInvocationID.xy), to_write);
                }
        "
    }
}
