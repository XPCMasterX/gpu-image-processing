use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::command_buffer::allocator::{
    StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo,
};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo, CopyImageToBufferInfo,
};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::{Device, DeviceCreateInfo, QueueCreateInfo};
use vulkano::format::Format;
use vulkano::image::view::ImageViewCreateInfo;
use vulkano::image::ImageSubresourceRange;
use vulkano::image::{view::ImageView, ImageDimensions, StorageImage};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::memory::allocator::{GenericMemoryAllocatorCreateInfo, StandardMemoryAllocator};
use vulkano::pipeline::ComputePipeline;
use vulkano::pipeline::Pipeline;
use vulkano::pipeline::PipelineBindPoint;
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
    let decoder =
        png::Decoder::new(File::open("/home/varshith/Code/gpu-calc/src/image.png").unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();

    let mut bytes_rgba = Vec::new();
    let mut index = 1;
    // Turn buf from RGB to RGBA
    for (_, i) in buf.iter().enumerate() {
        if index == 3 {
            bytes_rgba.push(*i);
            bytes_rgba.push(255);
            index = 1;
        } else {
            bytes_rgba.push(*i);
            index += 1;
        }
    }
    let bytes = &bytes_rgba[..];
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
    let descriptor_set_allocator = StandardDescriptorSetAllocator::new(device.clone());

    let image = StorageImage::new(
        &standard_allocator,
        ImageDimensions::Dim2d {
            width: info.width,
            height: info.height,
            array_layers: 1,
        },
        Format::R8G8B8A8_UNORM,
        Some(queue.queue_family_index()),
    )
    .unwrap();

    let image_view = ImageView::new(
        image.clone(),
        ImageViewCreateInfo {
            format: Some(Format::R8G8B8A8_UNORM),
            subresource_range: ImageSubresourceRange::from_parameters(Format::R8G8B8A8_UNORM, 1, 1),
            ..Default::default()
        },
    )
    .unwrap();

    let img_buf = CpuAccessibleBuffer::from_iter(
        &standard_allocator,
        BufferUsage {
            transfer_src: true,
            ..Default::default()
        },
        false,
        bytes.iter().copied(),
    )
    .expect("failed to create buffer");

    let dest_buf = CpuAccessibleBuffer::from_iter(
        &standard_allocator,
        BufferUsage {
            transfer_dst: true,
            ..Default::default()
        },
        false,
        (0..info.width * info.height * 4).map(|_| 0u8),
    )
    .unwrap();

    let shader = cs::load(device.clone()).expect("failed to create shader module");
    let compute_pipeline = ComputePipeline::new(
        device.clone(),
        shader.entry_point("main").unwrap(),
        &(),
        None,
        |_| {},
    )
    .expect("failed to create compute pipeline");

    let layout = compute_pipeline.layout().set_layouts().get(0).unwrap();
    let set = PersistentDescriptorSet::new(
        &descriptor_set_allocator,
        layout.clone(),
        [WriteDescriptorSet::image_view(0, image_view.clone())],
    )
    .unwrap();

    let mut builder = AutoCommandBufferBuilder::primary(
        &command_buffer_allocator,
        queue.queue_family_index(),
        CommandBufferUsage::SimultaneousUse,
    )
    .unwrap();

    builder
        .bind_pipeline_compute(compute_pipeline.clone())
        .bind_descriptor_sets(
            PipelineBindPoint::Compute,
            compute_pipeline.layout().clone(),
            0,
            set.clone(),
        )
        .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(img_buf, image.clone()))
        .unwrap()
        .dispatch([info.width / 8, info.height / 8, 1])
        .unwrap()
        .copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
            image.clone(),
            dest_buf.clone(),
        ))
        .unwrap();

    let command_buffer = builder.build().unwrap();

    let future = sync::now(device.clone())
        .then_execute(queue.clone(), command_buffer)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();

    future.wait(None).unwrap();

    let write_buf = dest_buf.read().unwrap();

    // Encode PNG
    let path = Path::new(r"2result.png");
    let file = File::create(path).unwrap();
    let ref mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, info.width, info.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&*write_buf).unwrap();
}
