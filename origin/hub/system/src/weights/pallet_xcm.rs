use core::marker::PhantomData;
use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_xcm::WeightInfo for WeightInfo<T> {
	fn send() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn teleport_assets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn reserve_transfer_assets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn transfer_assets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn execute() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn force_xcm_version() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn force_default_xcm_version() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn force_subscribe_version_notify() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn force_unsubscribe_version_notify() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn force_suspension() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn migrate_supported_version() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn migrate_version_notifiers() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn already_notified_target() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn notify_current_targets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn notify_target_migration_fail() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn migrate_version_notify_targets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn migrate_and_notify_old_targets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn new_query() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn take_response() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn claim_assets() -> Weight {
		Weight::from_parts(100_000_000, 0)
	}
	fn add_authorized_alias() -> Weight {
		Weight::from_parts(100_000, 0)
	}
	fn remove_authorized_alias() -> Weight {
		Weight::from_parts(100_000, 0)
	}
	fn weigh_message() -> Weight {
		Weight::from_parts(100_000, 0)
	}
}
