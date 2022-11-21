use std::sync::Arc;

use vulkano::{descriptor_set::allocator::StandardDescriptorSetAllocator, device::Device};

pub fn create_descriptor_set_allocator(device: Arc<Device>) -> StandardDescriptorSetAllocator {
    StandardDescriptorSetAllocator::new(device.clone())
}
