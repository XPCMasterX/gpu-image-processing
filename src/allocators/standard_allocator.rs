use std::sync::Arc;

use vulkano::{
    device::Device,
    memory::allocator::{GenericMemoryAllocatorCreateInfo, StandardMemoryAllocator},
};

pub fn create_standard_allocator(device: Arc<Device>) -> StandardMemoryAllocator {
    StandardMemoryAllocator::new(
        device.clone(),
        GenericMemoryAllocatorCreateInfo {
            block_sizes: &[(0, 1024)],
            ..Default::default()
        },
    )
    .unwrap()
}
