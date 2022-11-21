use std::sync::Arc;

use vulkano::{
    command_buffer::allocator::{
        StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo,
    },
    device::Device,
};

pub fn create_command_buffer_allocator(device: Arc<Device>) -> StandardCommandBufferAllocator {
    StandardCommandBufferAllocator::new(
        device.clone(),
        StandardCommandBufferAllocatorCreateInfo {
            ..Default::default()
        },
    )
}
