use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuCoord {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuRing {
    pub start_idx: u32,
    pub length: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuPoint {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuResult {
    pub is_inside: u32, // 1 for true, 0 for false
}

const SHADER: &str = r#"
struct GpuCoord {
    x: f32,
    y: f32,
};

struct GpuRing {
    start_idx: u32,
    length: u32,
};

struct GpuPoint {
    x: f32,
    y: f32,
};

struct GpuResult {
    is_inside: u32,
};

@group(0) @binding(0) var<storage, read> coords: array<GpuCoord>;
@group(0) @binding(1) var<storage, read> rings: array<GpuRing>;
@group(0) @binding(2) var<storage, read> points: array<GpuPoint>;
@group(0) @binding(3) var<storage, read_write> results: array<GpuResult>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let point_idx = global_id.x;
    if (point_idx >= arrayLength(&points)) {
        return;
    }

    let p = points[point_idx];
    let ring = rings[point_idx]; // Assuming 1:1 mapping between point and ring to check

    var crossings: u32 = 0u;
    let end_idx = ring.start_idx + ring.length;

    for (var i: u32 = ring.start_idx; i < end_idx - 1u; i = i + 1u) {
        let p1 = coords[i];
        let p2 = coords[i + 1u];

        // Ray casting point-in-polygon logic
        let y_cond = (p1.y > p.y) != (p2.y > p.y);
        if (y_cond) {
            let intersect_x = (p2.x - p1.x) * (p.y - p1.y) / (p2.y - p1.y) + p1.x;
            if (p.x < intersect_x) {
                crossings = crossings + 1u;
            }
        }
    }

    if (crossings % 2u != 0u) {
        results[point_idx].is_inside = 1u;
    } else {
        results[point_idx].is_inside = 0u;
    }
}
"#;

pub struct GpuContainmentContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}

impl GpuContainmentContext {
    pub fn new() -> Option<Self> {
        pollster::block_on(Self::init())
    }

    async fn init() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .ok()?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Containment Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER)),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Containment Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Some(Self {
            device,
            queue,
            pipeline,
        })
    }

    pub fn check_containment(
        &self,
        flat_coords: &[GpuCoord],
        rings: &[GpuRing],
        points: &[GpuPoint],
    ) -> Vec<bool> {
        if points.is_empty() {
            return Vec::new();
        }

        let coords_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Coords Buffer"),
                contents: bytemuck::cast_slice(flat_coords),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let rings_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Rings Buffer"),
                contents: bytemuck::cast_slice(rings),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let points_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Points Buffer"),
                contents: bytemuck::cast_slice(points),
                usage: wgpu::BufferUsages::STORAGE,
            });

        // Initialize results to 0
        let initial_results = vec![GpuResult { is_inside: 0 }; points.len()];
        let results_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Results Buffer"),
                contents: bytemuck::cast_slice(&initial_results),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (points.len() * std::mem::size_of::<GpuResult>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = self.pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Containment Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: coords_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: rings_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: points_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: results_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            let workgroup_count = (points.len() as u32).div_ceil(64);
            cpass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        encoder.copy_buffer_to_buffer(
            &results_buffer,
            0,
            &staging_buffer,
            0,
            (points.len() * std::mem::size_of::<GpuResult>()) as wgpu::BufferAddress,
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) =
            futures_channel::oneshot::channel::<Result<(), wgpu::BufferAsyncError>>();
        buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

        // In wgpu v22.0.0 device polling is executed using `device.poll(wgpu::MaintainBase::Wait)`
        self.device.poll(wgpu::MaintainBase::Wait);

        pollster::block_on(async {
            receiver.await.unwrap().unwrap();
        });

        let data = buffer_slice.get_mapped_range();
        let result_slice: &[GpuResult] = bytemuck::cast_slice(&data);

        let mut final_results = Vec::with_capacity(points.len());
        for res in result_slice {
            final_results.push(res.is_inside != 0);
        }

        drop(data);
        staging_buffer.unmap();

        final_results
    }
}
