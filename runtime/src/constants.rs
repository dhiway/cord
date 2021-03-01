// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! A set of constant values used in runtime.

/// Money matters.
pub mod currency {
	use cord_primitives::Balance;

	pub const CRD: Balance = 1_000_000_000_000_000_000;
	pub const RUPEES: Balance = CRD;
	pub const PAISE: Balance = RUPEES / 100;
	pub const ANNAPAISE: Balance = PAISE / 100;
	pub const MILLIPAISE: Balance = ANNAPAISE / 100;

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 20 * RUPEES + (bytes as Balance) * 100 * MILLIPAISE
	}
}

/// Time and blocks.
pub mod time {
	use cord_primitives::{BlockNumber, Moment};
	pub const MILLISECS_PER_BLOCK: Moment = 4000;
	pub const SECS_PER_BLOCK: Moment = MILLISECS_PER_BLOCK / 1000;

	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
	pub const EPOCH_DURATION_IN_BLOCKS: BlockNumber = 4 * HOURS;

	pub const EPOCH_DURATION_IN_SLOTS: u64 = {
		const SLOT_FILL_RATE: f64 = MILLISECS_PER_BLOCK as f64 / SLOT_DURATION as f64;

		(EPOCH_DURATION_IN_BLOCKS as f64 * SLOT_FILL_RATE) as u64
	};

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60 / (SECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;
}
