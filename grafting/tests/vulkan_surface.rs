// SPDX-License-Identifier: MPL-2.0
#![cfg(all(target_os = "linux", feature = "wgpu-30"))]
use grafting::wgpu;
use grafting::{
    FrameSize, HostWgpuContext,
    vulkan_dmabuf::create_dmabuf_host_context,
    vulkan_surface::{SharedVulkanSurface, import_surface},
};
use std::time::{Duration, Instant};
fn wait(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !ready() {
        assert!(Instant::now() < deadline, "Vulkan surface deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn read(host: &HostWgpuContext, texture: &wgpu::Texture) -> Vec<u8> {
    let stride = 256;
    let buffer = host.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("native qualification readback"),
        size: stride * 16,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = host.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride as u32),
                rows_per_image: Some(16),
            },
        },
        texture.size(),
    );
    host.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    wait(|| {
        host.device.poll(wgpu::PollType::Poll).unwrap();
        match rx.try_recv() {
            Ok(r) => {
                r.unwrap();
                true
            },
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(e) => panic!("{e}"),
        }
    });
    let mapped = buffer.slice(..).get_mapped_range().unwrap();
    let bytes = mapped
        .chunks(stride as usize)
        .flat_map(|row| row[..64].to_vec())
        .collect();
    drop(mapped);
    buffer.unmap();
    bytes
}
#[test]
#[ignore = "requires hardware Vulkan with linear RGBA8 DMA-BUF export/import"]
fn native_storage_round_trip_and_reuse_require_final_reference_release() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        flags: wgpu::InstanceFlags::default(),
        memory_budget_thresholds: Default::default(),
        backend_options: Default::default(),
        display: None,
    });
    let adapter = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN))
        .into_iter()
        .find(|a| {
            matches!(
                a.get_info().device_type,
                wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::DiscreteGpu
            )
        })
        .expect("hardware Vulkan adapter");
    eprintln!("Graft Vulkan surface adapter: {:?}", adapter.get_info());
    let producer = create_dmabuf_host_context(&adapter, &Default::default()).unwrap();
    let consumer = create_dmabuf_host_context(&adapter, &Default::default()).unwrap();
    let source = producer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("local source"),
        size: wgpu::Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut surface = SharedVulkanSurface::new(&producer, FrameSize::new(16, 16)).unwrap();
    assert!(surface.export_fd().is_err());
    for color in [[128u8, 0, 0, 128], [0, 0, 255, 255]] {
        let bytes: Vec<_> = (0..256).flat_map(|_| color).collect();
        producer.queue.write_texture(
            source.as_image_copy(),
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(64),
                rows_per_image: Some(16),
            },
            source.size(),
        );
        // SAFETY: source was created on this surface's exact producer device.
        unsafe {
            surface.copy_from(&source).unwrap();
            assert!(surface.copy_from(&source).is_err());
        }
        wait(|| surface.poll_ready().unwrap());
        let fd = surface.export_fd().unwrap();
        let (texture, mut release) = import_surface(
            fd,
            surface.size(),
            surface.offset(),
            surface.stride(),
            &consumer,
        )
        .unwrap();
        assert_eq!(read(&consumer, &texture), bytes);
        let view = texture.create_view(&Default::default());
        release.seal().unwrap();
        assert!(release.seal().is_err());
        drop(texture);
        for _ in 0..20 {
            assert!(
                !release.poll_released().unwrap(),
                "view retains backing after GPU completion"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        drop(view);
        wait(|| release.poll_released().unwrap());
        // SAFETY: importer completed FOREIGN release and all aliases are gone.
        unsafe {
            surface.acknowledge_release().unwrap();
            assert!(surface.acknowledge_release().is_err());
        }
    }
}
