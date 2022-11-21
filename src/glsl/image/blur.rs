pub mod blur {
    vulkano_shaders::shader! {
        ty: "compute",
        src: "
                #version 450
                layout(local_size_x = 4, local_size_y = 4, local_size_z = 1) in;
                
                layout(set = 0, binding = 0, rgba8) uniform image2D img;
                
                void main() {
                    int blur_kernel = 3;
                    float divide = 0;

                    float red_sum = 0;
                    float green_sum = 0;
                    float blue_sum = 0;

                    for (int i = 0; i < blur_kernel; i++) {
                        // left
                        vec4 data = imageLoad(img, ivec2(gl_GlobalInvocationID.x - i, gl_GlobalInvocationID.y));
                        red_sum += data[0];
                        green_sum += data[1];
                        blue_sum += data[2];
                        divide += 1.0;

                        // right
                        data = imageLoad(img, ivec2(gl_GlobalInvocationID.x + i, gl_GlobalInvocationID.y));
                        red_sum += data[0];
                        green_sum += data[1];
                        blue_sum += data[2];
                        divide += 1.0;

                        // top
                        data = imageLoad(img, ivec2(gl_GlobalInvocationID.x, gl_GlobalInvocationID.y  - i));
                        red_sum += data[0];
                        green_sum += data[1];
                        blue_sum += data[2];
                        divide += 1.0;

                        // bottom
                        data = imageLoad(img, ivec2(gl_GlobalInvocationID.x, gl_GlobalInvocationID.y  + i));
                        red_sum += data[0];
                        green_sum += data[1];
                        blue_sum += data[2];
                        divide += 1.0;

                        /* Area within quadrant */
                        for (int j = 0; j < blur_kernel; j++) {
                            // top left
                            vec4 data2 = imageLoad(img, ivec2(gl_GlobalInvocationID.x - j, gl_GlobalInvocationID.y  - i));
                            red_sum += data[0];
                            green_sum += data[1];
                            blue_sum += data[2];
                            divide += 1.0;

                            // top right
                            data2 = imageLoad(img, ivec2(gl_GlobalInvocationID.x + j, gl_GlobalInvocationID.y  - i));
                            red_sum += data[0];
                            green_sum += data[1];
                            blue_sum += data[2];
                            divide += 1.0;

                            // bottom left
                            data = imageLoad(img, ivec2(gl_GlobalInvocationID.x - j, gl_GlobalInvocationID.y + i));
                            red_sum += data[0];
                            green_sum += data[1];
                            blue_sum += data[2];
                            divide += 1.0;

                            // bottom right
                            data = imageLoad(img, ivec2(gl_GlobalInvocationID.x + j, gl_GlobalInvocationID.y + i));
                            red_sum += data[0];
                            green_sum += data[1];
                            blue_sum += data[2];
                            divide += 1.0;
                        }
                    }

                    // calculate avg
                    red_sum = red_sum / divide;
                    green_sum = green_sum / divide;
                    blue_sum = blue_sum / divide;

                    vec4 to_write = vec4(red_sum, green_sum, blue_sum, 1.0);
                    
                    imageStore(img, ivec2(gl_GlobalInvocationID.xy), to_write);
                }
        "
    }
}
