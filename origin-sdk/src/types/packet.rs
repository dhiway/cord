pub use origin_primitives::{PacketMetadataView, PacketPointer, PacketStateView, PacketStatus};

#[derive(Clone, Debug)]
pub struct PacketTimelineView {
	pub events: Vec<String>,
}
