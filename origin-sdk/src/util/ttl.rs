use std::time::{Duration, SystemTime};

/// Simple TTL helper for authorization payloads.
pub fn expires_in(ttl: Duration) -> u64 {
	let now = SystemTime::now()
		.duration_since(SystemTime::UNIX_EPOCH)
		.unwrap_or_default()
		.as_secs();
	now + ttl.as_secs()
}
