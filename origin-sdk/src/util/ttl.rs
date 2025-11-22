/// Compute the block height at which a payload signed at `current_block` expires.
///
/// Pallet TTL checks are block-based, so avoid using wall-clock time when
/// constructing authorization payloads.
pub fn expires_at(current_block: u32, ttl_blocks: u32) -> u32 {
	current_block.saturating_add(ttl_blocks)
}
