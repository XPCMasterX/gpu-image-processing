use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo, CopyImageToBufferInfo,
};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::{Device, DeviceCreateInfo, Queue, QueueCreateInfo};
use vulkano::format::Format;
use vulkano::image::view::ImageViewCreateInfo;
use vulkano::image::ImageSubresourceRange;
use vulkano::image::{view::ImageView, ImageDimensions, StorageImage};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::pipeline::ComputePipeline;
use vulkano::pipeline::Pipeline;
use vulkano::pipeline::PipelineBindPoint;
use vulkano::sync::{self, GpuFuture};
use vulkano::VulkanLibrary;

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

mod allocators;
mod glsl;

use crate::allocators::*;
use crate::glsl::image::invert;
fn main() {
    // #region PNG
    let decode_png_start = Instant::now();
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
    let decode_png_duration = decode_png_start.elapsed();
    println!("Time to decode PNG: {:?}", decode_png_duration);
    // #endregion
    let gpu_start = Instant::now();

    let (device, mut queues) = vulkan_init();

    // Unwrap queues (there can be multiple queues but here only 1 is requested)
    let queue = queues.next().unwrap();

    // Create allocators
    let standard_allocator = standard_allocator::create_standard_allocator(device.clone());
    let command_buffer_allocator =
        command_buffer_allocator::create_command_buffer_allocator(device.clone());
    let descriptor_set_allocator =
        descriptor_set_allocator::create_descriptor_set_allocator(device.clone());

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

    let shader = invert::invert::load(device.clone()).expect("failed to create shader module");
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
        .dispatch([info.width / 4, info.height / 4, 1])
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
    let gpu_duration = gpu_start.elapsed();
    println!("Time for GPU: {:?} ", gpu_duration);

    // Encode PNG
    let encode_start = Instant::now();
    let path = Path::new(r"2result.png");
    let file = File::create(path).unwrap();
    let ref mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, info.width, info.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&*write_buf).unwrap();
    let encode_duration = encode_start.elapsed();
    println!("Time to encode PNG: {:?}", encode_duration);
}

fn vulkan_init() -> (Arc<Device>, impl ExactSizeIterator<Item = Arc<Queue>>) {
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
    println!(
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
    let (device, queues) = Device::new(
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

    (device, queues)
}
