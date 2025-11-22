pub use origin_primitives::{PacketMetadataView, PacketPointer, PacketStateView, PacketStatus};

use codec::Decode;

#[derive(Clone, Debug, Decode)]
pub struct PacketTimelineView {
	pub events: Vec<String>,
}
