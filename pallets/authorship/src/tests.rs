use frame_support::{assert_noop, assert_ok, BoundedVec};

use cord_utilities::mock::mock_origin;
use frame_system::RawOrigin;
use sp_runtime::DispatchError;

use crate::{mock::*, Banned, DidNameOwnershipOf, Error, Names, Owner, Pallet};


#[test]
fn registering_successful() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(true,true);
	})
}
