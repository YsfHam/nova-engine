use std::collections::HashMap;

use crate::graphics::buffer::{DynamicBuffer, Offset};

/// A reference to shared, persistent geometry in the [`GeometryPool`].
///
/// Created once via [`GeometryPool::insert`], then reused across frames and
/// across batches. The geometry is uploaded to a **persistent** GPU buffer
/// (separate from the per-frame staging buffer) and its offsets never change.
///
/// `GeometryRef` is a cheap `Copy` handle (a `u64` id). Multiple `DrawBatch`es
/// can reference the same `GeometryRef` — they all bind the same GPU buffer
/// slices, so the geometry is uploaded exactly once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GeometryRef {
    id: u64,
}

/// Persistent, append-only storage for shared geometry.
///
/// Lives on [`RenderContext`](crate::graphics::render::RenderContext). The
/// backing [`DynamicBuffer`] is **never cleared** — geometry stays at its
/// assigned offset forever (or until [`compact`](Self::compact)). This means
/// `GeometryRef` offsets are permanent: no invalidation on frame reset, no
/// ring-slot tracking.
///
/// # Fragmentation
///
/// The buffer is append-only, so normal operation creates no gaps. Gaps only
/// appear when a [`GeometryRef`] is removed (via [`remove`](Self::remove)).
/// The wasted bytes are tracked; when they exceed a threshold, call
/// [`compact`](Self::compact) to rebuild the buffer contiguously.
///
/// # Separate from the staging buffer
///
/// The per-frame staging buffer (`StagingBufferPool`) is ring-buffered and
/// cleared every frame — it holds dynamic data. `GeometryPool` holds static
/// data in a separate, persistent buffer. The two never mix, so ring resets
/// never invalidate shared geometry offsets.
#[allow(dead_code)] // remove/needs_compaction/compact are V1 API surface
pub(crate) struct GeometryPool {
    buffer: DynamicBuffer,
    entries: HashMap<u64, GeometryEntry>,
    next_id: u64,
    /// Total bytes occupied by removed entries (for compaction threshold).
    wasted_bytes: u64,
}

#[allow(dead_code)]
struct GeometryEntry {
    vertex_offset: Offset,
    index_offset: Offset,
    /// Total bytes this entry occupies in the buffer (for compaction).
    total_size: u64,
}

#[allow(dead_code)] // remove/needs_compaction/compact are V1 API surface
impl GeometryPool {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let buffer = DynamicBuffer::new(
            device,
            "GeometryPool buffer",
            1024, // start small — grows on demand
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::INDEX,
        );
        Self {
            buffer,
            entries: HashMap::new(),
            next_id: 0,
            wasted_bytes: 0,
        }
    }

    /// Inserts shared geometry (vertices + indices) into the persistent buffer.
    ///
    /// The data is uploaded immediately via `queue.write_buffer`. The returned
    /// [`GeometryRef`] can be used in any `DrawBatch` via
    /// [`DrawBatch::with_shared_geometry`](crate::graphics::draw_batch::DrawBatch::with_shared_geometry).
    ///
    /// Call this once (at init, when a mesh loads, etc.) — not per frame.
    pub fn insert(
        &mut self,
        vertices: &[u8],
        indices: &[u16],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) -> GeometryRef {
        let id = self.next_id;
        self.next_id += 1;

        let vertex_offset = self.buffer.extend(vertices, device, queue, encoder);
        let index_offset = self
            .buffer
            .extend(bytemuck::cast_slice(indices), device, queue, encoder);

        let total_size = vertex_offset.size + index_offset.size;

        self.entries.insert(
            id,
            GeometryEntry {
                vertex_offset,
                index_offset,
                total_size,
            },
        );

        GeometryRef { id }
    }

    /// Returns the permanent vertex + index offsets for this geometry.
    ///
    /// Returns `None` if the `GeometryRef` was removed or never inserted.
    pub fn offsets(&self, geo: GeometryRef) -> Option<(Offset, Offset)> {
        self.entries
            .get(&geo.id)
            .map(|e| (e.vertex_offset, e.index_offset))
    }

    /// The persistent GPU buffer holding all shared geometry. Bind slices
    /// from this buffer during the render pass.
    pub(crate) fn buffer(&self) -> &wgpu::Buffer {
        self.buffer.buffer()
    }

    /// Removes a `GeometryRef`, marking its bytes as wasted. The data stays
    /// in the buffer until compaction.
    pub fn remove(&mut self, geo: GeometryRef) {
        if let Some(entry) = self.entries.remove(&geo.id) {
            self.wasted_bytes += entry.total_size;
        }
    }

    /// Whether the wasted bytes exceed `threshold` (e.g. 25% of buffer size).
    /// If so, call [`compact`](Self::compact) to reclaim the space.
    pub fn needs_compaction(&self, threshold: u64) -> bool {
        self.wasted_bytes > threshold
    }

/// Rebuilds the buffer contiguously, removing gaps from removed entries.
    /// Updates all live `GeometryRef` offsets.
    ///
    /// **Not implemented in V1.** The buffer is append-only and the wasted
    /// space from removed entries is typically negligible (a few KB). If
    /// `needs_compaction` returns true, the simplest action is to drop and
    /// recreate the `GeometryPool`, re-inserting all live geometry — but this
    /// requires the caller to still hold the source vertex/index data.
    pub(crate) fn compact(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
    ) {
        // V1: no-op. The buffer keeps its high-water mark. Wasted bytes from
        // removed entries are tracked but not reclaimed.
    }
}