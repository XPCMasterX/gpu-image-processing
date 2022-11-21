use bytemuck::{Pod, Zeroable};
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::command_buffer::allocator::{
    CommandBufferAllocator, StandardCommandBufferAllocator,
    StandardCommandBufferAllocatorCreateInfo,
};
use vulkano::command_buffer::{
    self, AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
    CopyImageToBufferInfo,
};
use vulkano::command_buffer::{RenderPassBeginInfo, SubpassContents};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::{Device, DeviceCreateInfo, QueueCreateInfo};
use vulkano::format::Format;
use vulkano::image::view::ImageViewCreateInfo;
use vulkano::image::ImageSubresourceRange;
use vulkano::image::{view::ImageView, ImageDimensions, StorageImage};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::memory::allocator::{GenericMemoryAllocatorCreateInfo, StandardMemoryAllocator};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::render_pass;
use vulkano::pipeline::graphics::vertex_input::{BuffersDefinition, Vertex};
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::pipeline::Pipeline;
use vulkano::pipeline::PipelineBindPoint;
use vulkano::pipeline::{self, ComputePipeline};
use vulkano::render_pass::Subpass;
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo};
use vulkano::sync::{self, GpuFuture};
use vulkano::VulkanLibrary;

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

mod cs {
    vulkano_shaders::shader! {
        ty: "compute",
        src: "
#version 450
                layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;
                
                layout(set = 0, binding = 0, rgba8) uniform image2D img;
                
                void main() {
                    vec4 data = imageLoad(img, ivec2(gl_GlobalInvocationID.xy));
                    float avg = (data[0] + data[1] + data[2]) / 3.0;  
                    vec4 to_write = vec4(avg, avg, avg, 1.0);
                    imageStore(img, ivec2(gl_GlobalInvocationID.xy), to_write);
                }
        "
    }
}

fn main() {
    // #region PNG
    // Decode PNG
    // let decoder =
    //     png::Decoder::new(File::open("/home/varshith/Code/gpu-calc/src/image.png").unwrap());
    // let mut reader = decoder.read_info().unwrap();
    // let mut buf = vec![0; reader.output_buffer_size()];
    // let info = reader.next_frame(&mut buf).unwrap();

    // let mut bytes_rgba = Vec::new();
    // let mut index = 1;
    // // Turn buf from RGB to RGBA
    // for (_, i) in buf.iter().enumerate() {
    //     if index == 3 {
    //         bytes_rgba.push(*i);
    //         bytes_rgba.push(255);
    //         index = 1;
    //     } else {
    //         bytes_rgba.push(*i);
    //         index += 1;
    //     }
    // }
    // let bytes = &bytes_rgba[..];
    // #endregion
    // Link to local vulkan library
    let library = VulkanLibrary::new().expect("No local Vulkan library found.");
    let instance =
        Instance::new(library, InstanceCreateInfo::default()).expect("Failed to create instance.");

    // Chose physical device to use
    let physical = instance
        .enumerate_physical_devices()
        .expect("Could not enumerate devices")
        .next()
        .expect("No devices that support Vulkan found");

    // Print properties
    let properties = physical.properties();
    print!(
        "Device: {}, Type: {:?}, Driver: {:?}, {:?} ",
        properties.device_name,
        properties.device_type,
        properties.driver_name.as_ref().unwrap(),
        properties.driver_info.as_ref().unwrap()
    );

    // Find queue supporting graphical operations
    let queue_family_index = physical
        .queue_family_properties()
        .iter()
        .enumerate()
        .position(|(_, q)| q.queue_flags.graphics)
        .expect("couldn't find a graphical queue family") as u32;

    // Create device
    let (device, mut queues) = Device::new(
        physical,
        DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .expect("Failed to create device");

    // Unwrap queues (there can be multiple queues but here only 1 is requested)
    let queue = queues.next().unwrap();

    // Create allocators
    let standard_allocator = StandardMemoryAllocator::new(
        device.clone(),
        GenericMemoryAllocatorCreateInfo {
            block_sizes: &[(0, 1024)],
            ..Default::default()
        },
    )
    .unwrap();
    let command_buffer_allocator = StandardCommandBufferAllocator::new(
        device.clone(),
        StandardCommandBufferAllocatorCreateInfo {
            ..Default::default()
        },
    );

    // Load shaders
    mod vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            src: "
#version 450

layout(location = 0) in vec2 position;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
}"
        }
    }

    mod fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            src: "
#version 450

layout(location = 0) out vec4 f_color;

void main() {
    f_color = vec4(1.0, 0.0, 0.0, 1.0);
}"
        }
    }

    let vs = vs::load(device.clone()).expect("failed to create shader module");
    let fs = fs::load(device.clone()).expect("failed to create shader module");

    let image = StorageImage::new(
        &standard_allocator,
        ImageDimensions::Dim2d {
            width: 1024,
            height: 1024,
            array_layers: 1,
        },
        Format::R8G8B8A8_UNORM,
        Some(queue.queue_family_index()),
    )
    .unwrap();

    let buf = CpuAccessibleBuffer::from_iter(
        &standard_allocator,
        BufferUsage {
            transfer_dst: true,
            ..Default::default()
        },
        false,
        (0..1024 * 1024 * 4).map(|_| 0u8),
    )
    .expect("failed to create buffer");

    // Create vertexes
    #[repr(C)]
    #[derive(Default, Copy, Clone, Zeroable, Pod)]
    struct Vertex {
        position: [f32; 2],
    }

    let vertex1 = Vertex {
        position: [-0.5, -0.5],
    };
    let vertex2 = Vertex {
        position: [0.0, -1.0],
    };
    let vertex3 = Vertex {
        position: [0.5, -0.5],
    };

    vulkano::impl_vertex!(Vertex, position);

    let vertex_buffer = CpuAccessibleBuffer::from_iter(
        &standard_allocator,
        BufferUsage {
            vertex_buffer: true,
            ..Default::default()
        },
        false,
        vec![vertex1, vertex2, vertex3].into_iter(),
    )
    .unwrap();

    let render_pass = vulkano::single_pass_renderpass!(device.clone(),
        attachments: {
            color: {
                load: Clear,
                store: Store,
                format: Format::R8G8B8A8_UNORM,
                samples: 1,
            }
        },
        pass: {
            color: [color],
            depth_stencil: {}
        }
    )
    .unwrap();

    let view = ImageView::new_default(image.clone()).unwrap();
    let framebuffer = Framebuffer::new(
        render_pass.clone(),
        FramebufferCreateInfo {
            attachments: vec![view],
            ..Default::default()
        },
    )
    .unwrap();

    let mut builder = AutoCommandBufferBuilder::primary(
        &command_buffer_allocator,
        queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [1024.0, 1024.0],
        depth_range: 0.0..1.0,
    };

    let pipeline = GraphicsPipeline::start()
        .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
        .vertex_shader(vs.entry_point("main").unwrap(), ())
        .input_assembly_state(InputAssemblyState::new())
        .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([viewport]))
        .fragment_shader(fs.entry_point("main").unwrap(), ())
        .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
        .build(device.clone())
        .unwrap();

    builder
        .begin_render_pass(
            RenderPassBeginInfo {
                clear_values: vec![Some([0.0, 0.0, 1.0, 1.0].into())],
                ..RenderPassBeginInfo::framebuffer(framebuffer.clone())
            },
            SubpassContents::Inline,
        )
        .unwrap()
        .bind_pipeline_graphics(pipeline.clone())
        .bind_vertex_buffers(0, vertex_buffer.clone())
        .draw(3, 1, 0, 0)
        .unwrap()
        .end_render_pass()
        .unwrap()
        .copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
            image.clone(),
            buf.clone(),
        ))
        .unwrap();

    let command_buffer = builder.build().unwrap();
    let future = sync::now(device.clone())
        .then_execute(queue.clone(), command_buffer)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();
    future.wait(None).unwrap();

    let buffer_content = buf.read().unwrap();
    // Encode PNG
    let path = Path::new(r"2result.png");
    let file = File::create(path).unwrap();
    let ref mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, 1024, 1024);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&*buffer_content).unwrap();
}
