#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(any(feature = "mock", test))]
pub mod mock;

#[cfg(test)]
pub mod tests;

use cord_primitives::StatusOf;
use frame_support::{ensure, storage::types::StorageMap};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::{prelude::Clone, str};
pub mod types;
pub mod weights;
pub use crate::{pallet::*, types::*, weights::WeightInfo};
use frame_system::pallet_prelude::BlockNumberFor;
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use sp_runtime::SaturatedConversion;
use sp_std::{vec, vec::Vec};

#[frame_support::pallet]
pub mod pallet {

	use super::*;
	use cord_utilities::traits::CallSources;
	use frame_support::pallet_prelude::{OptionQuery, *};
	use frame_system::pallet_prelude::*;
	pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};
	use sp_runtime::traits::Hash;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Space Identifier
	pub type SpaceIdOf = Ss58Identifier;
	/// Statement Identifier
	pub type UseOnceStatementIdOf = Ss58Identifier;
	/// Schema Identifier
	pub type SchemaIdOf = Ss58Identifier;
	/// Authorization Identifier
	pub type AuthorizationIdOf = Ss58Identifier;
	/// Type of a creator identifier.
	pub type StatementCreatorOf<T> = pallet_chain_space::SpaceCreatorOf<T>;
	/// Hash of the statement.
	pub type StatementDigestOf<T> = <T as frame_system::Config>::Hash;
	/// Type of the identitiy.
	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
	/// Type for the statement details
	pub type UseOnceStatementDetailsOf<T> =
		StatementDetails<StatementDigestOf<T>, SchemaIdOf, SpaceIdOf>;
	/// Type for the statement entry details
	pub type UseOnceStatementEntryStatusOf<T> =
		StatementEntryStatus<StatementCreatorOf<T>, StatusOf>;
	/// Type for the statement entry details
	pub type UseOnceStatementPresentationDetailsOf<T> = StatementPresentationDetails<
		StatementCreatorOf<T>,
		PresentationTypeOf,
		StatementDigestOf<T>,
		SpaceIdOf,
	>;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_chain_space::Config + identifier::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, StatementCreatorOf<Self>>;
		/// Maximum entires supported per batch call
		#[pallet::constant]
		type MaxDigestsPerBatch: Get<u16>;
		/// Maximum removals per call
		#[pallet::constant]
		type MaxRemoveEntries: Get<u16>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Use-once statement identifiers stored on chain.
	/// It maps from an identifier to its details.
	/// Only stores the latest state.
	#[pallet::storage]
	pub type UseOnceStatements<T> = StorageMap<
		_,
		Blake2_128Concat,
		UseOnceStatementIdOf,
		UseOnceStatementDetailsOf<T>,
		OptionQuery,
	>;

	/// Use-once statement uniques stored on chain.
	/// It maps from a statement identifier and hash to its details.
	#[pallet::storage]
	pub type UseOnceEntries<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		UseOnceStatementIdOf,
		Blake2_128Concat,
		StatementDigestOf<T>,
		StatementCreatorOf<T>,
		OptionQuery,
	>;

	/// Use-once statement presentations stored on chain.
	/// It maps from a statement identifier and hash to its details.
	#[pallet::storage]
	pub type UseOncePresentations<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		UseOnceStatementIdOf,
		Blake2_128Concat,
		StatementDigestOf<T>,
		UseOnceStatementPresentationDetailsOf<T>,
		OptionQuery,
	>;

	/// Revocation registry of use-once statement entries stored on chain.
	/// It maps from a statement identifier and hash to its details.
	#[pallet::storage]
	pub type UseOnceRevocationList<T> = StorageDoubleMap<
		_,
		Twox64Concat,
		UseOnceStatementIdOf,
		Blake2_128Concat,
		StatementDigestOf<T>,
		UseOnceStatementEntryStatusOf<T>,
		OptionQuery,
	>;

	/// Storage for Identifier lookup.
	/// It maps from a statement entry digest and registry id to an identifier.
	#[pallet::storage]
	pub type UseOnceIdentifierLookup<T> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		StatementDigestOf<T>,
		Twox64Concat,
		SpaceIdOf,
		UseOnceStatementIdOf,
		OptionQuery,
	>;

	/// Storage to track used statements.
	#[pallet::storage]
	pub type UsedStatements<T> =
		StorageMap<_, Blake2_128Concat, UseOnceStatementIdOf, bool, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new use-once statement identifier has been registered.
		/// \[statement identifier, statement digest, controller\]
		Register {
			identifier: UseOnceStatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorOf<T>,
		},
		/// A use-once statement identifier has been updated.
		/// \[statement identifier, digest, controller\]
		Update {
			identifier: UseOnceStatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorOf<T>,
		},
		/// A use-once statement identifier status has been revoked.
		/// \[statement identifier, controller\]
		Revoke { identifier: UseOnceStatementIdOf, author: StatementCreatorOf<T> },
		/// A use-once statement identifier status has been restored.
		/// \[statement identifier, controller\]
		Restore { identifier: UseOnceStatementIdOf, author: StatementCreatorOf<T> },
		/// A use-once statement identifier has been removed.
		/// \[statement identifier,  controller\]
		Remove { identifier: UseOnceStatementIdOf, author: StatementCreatorOf<T> },
		/// A use-once statement identifier has been removed.
		/// \[statement identifier,  controller\]
		PartialRemoval {
			identifier: UseOnceStatementIdOf,
			removed: u32,
			author: StatementCreatorOf<T>,
		},
		/// A use-once statement digest has been added.
		/// \[statement identifier, digest, controller\]
		PresentationAdded {
			identifier: UseOnceStatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorOf<T>,
		},
		/// A use-once statement digest has been added.
		/// \[statement identifier, digest, controller\]
		PresentationRemoved {
			identifier: UseOnceStatementIdOf,
			digest: StatementDigestOf<T>,
			author: StatementCreatorOf<T>,
		},
		/// A use-once statement batch has been processed.
		/// \[successful count, failed count, failed indices,
		/// controller\]
		RegisterBatch {
			successful: u32,
			failed: u32,
			indices: Vec<u16>,
			author: StatementCreatorOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Use-once statement idenfier is not unique
		UseOnceStatementAlreadyAnchored,
		/// Use-once statement idenfier not found
		UseOnceStatementNotFound,
		/// Only when the author is not the controller/delegate.
		UnauthorizedOperation,
		/// Use-once statement entry not found
		UseOnceStatementEntryNotFound,
		/// Use-once statement entry marked inactive
		UseOnceStatementRevoked,
		/// Use-once statement idenfier not marked inactive
		UseOnceStatementNotRevoked,
		/// Use-once statement link does not exist
		UseOnceStatementLinkNotFound,
		/// Use-once statement link is revoked
		UseOnceStatementLinkRevoked,
		/// Invalid creator signature
		InvalidSignature,
		/// Use-once statement hash is not unique
		HashAlreadyAnchored,
		/// Expired Tx Signature
		ExpiredSignature,
		/// Invalid Use-once Statement Identifier
		InvalidUseOnceStatementIdentifier,
		/// Invalid Schema Identifier Length
		InvalidIdentifierLength,
		/// Use-once statement not part of space
		UseOnceStatementSpaceMismatch,
		/// Use-once statement digest is not unique
		DigestHashAlreadyAnchored,
		/// Use-once statement digest has no schemas
		EmptySchemas,
		/// Entry is already part of the revocation list
		RevocationEntryExists,
		/// Entry is not part of the revocation list
		RevocationEntryNotFound,
		/// Identifier lookup mismatch
		IdentifierMismatch,
		/// Too many digests in a single batch
		TooManyDigestsInBatch,
		/// No identifier found in batch operation
		NoIdentifiersInBatch,
		/// Identifier entry is not unique
		IdentifierAlreadyAnchored,
		/// The statement has already been used
		UseOnceStatementAlreadyUsed,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register a new use-once statement identifier on chain.
		#[pallet::weight(T::WeightInfo::register())]
		pub fn register(
			origin: OriginFor<T>,
			identifier: UseOnceStatementIdOf,
			digest: StatementDigestOf<T>,
			schema: SchemaIdOf,
			space: SpaceIdOf,
		) -> DispatchResult {
			let who = T::EnsureOrigin::ensure_origin(origin)?;

			// Ensure the statement has not been used before
			ensure!(
				!UsedStatements::<T>::contains_key(&identifier),
				Error::<T>::UseOnceStatementAlreadyUsed
			);

			let details = UseOnceStatementDetailsOf::<T> {
				digest,
				schema,
				space,
				author: who.sender().clone(),
			};
			UseOnceStatements::<T>::insert(&identifier, &details);

			Self::deposit_event(Event::Register {
				identifier,
				digest,
				author: who.sender().clone(),
			});
			Ok(())
		}

		/// Revoke a use-once statement identifier
		#[pallet::weight(T::WeightInfo::revoke())]
		pub fn revoke(
			origin: OriginFor<T>,
			identifier: UseOnceStatementIdOf,
			digest: StatementDigestOf<T>,
		) -> DispatchResult {
			let who = T::EnsureOrigin::ensure_origin(origin)?;
			let details = UseOnceStatements::<T>::get(&identifier)
				.ok_or(Error::<T>::UseOnceStatementNotFound)?;

			// Ensure the statement is revoked
			ensure!(details.digest == digest, Error::<T>::UseOnceStatementEntryNotFound);
			let status = UseOnceStatementEntryStatusOf::<T> {
				author: who.sender().clone(),
				status: StatusOf::Revoked,
			};
			UseOnceRevocationList::<T>::insert(&identifier, &digest, &status);
			Self::deposit_event(Event::Revoke { identifier, author: who.sender().clone() });
			Ok(())
		}

		/// Remove a use-once statement identifier
		#[pallet::weight(T::WeightInfo::remove())]
		pub fn remove(
			origin: OriginFor<T>,
			identifier: UseOnceStatementIdOf,
			digest: StatementDigestOf<T>,
		) -> DispatchResult {
			let who = T::EnsureOrigin::ensure_origin(origin)?;
			let _ = UseOnceStatements::<T>::get(&identifier)
				.ok_or(Error::<T>::UseOnceStatementNotFound)?;

			let _ = UseOnceEntries::<T>::get(&identifier, &digest)
				.ok_or(Error::<T>::UseOnceStatementNotFound)?;
			UseOnceEntries::<T>::remove(&identifier, &digest);

			// Mark the statement as used
			UsedStatements::<T>::insert(&identifier, true);

			Self::deposit_event(Event::Remove { identifier, author: who.sender().clone() });
			Ok(())
		}

		/// Add a presentation to a use-once statement identifier
		#[pallet::weight(T::WeightInfo::presentation())]
		pub fn presentation(
			origin: OriginFor<T>,
			identifier: UseOnceStatementIdOf,
			digest: StatementDigestOf<T>,
			entry: UseOnceStatementPresentationDetailsOf<T>,
		) -> DispatchResult {
			let who = T::EnsureOrigin::ensure_origin(origin)?;

			// Ensure the statement has not been used before
			ensure!(
				!UsedStatements::<T>::contains_key(&identifier),
				Error::<T>::UseOnceStatementAlreadyUsed
			);

			let _ = UseOnceStatements::<T>::get(&identifier)
				.ok_or(Error::<T>::UseOnceStatementNotFound)?;

			UseOncePresentations::<T>::insert(&identifier, &digest, &entry);

			Self::deposit_event(Event::PresentationAdded {
				identifier,
				digest,
				author: who.sender().clone(),
			});
			Ok(())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate as pallet_use_once_statements;
	use frame_support::{assert_noop, assert_ok, parameter_types};
	use frame_system as system;
	use sp_core::H256;
	use sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
	};

	type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
	type Block = frame_system::mocking::MockBlock<Test>;

	frame_support::construct_runtime!(
		pub enum Test where
			Block = Block,
			NodeBlock = Block,
			UncheckedExtrinsic = UncheckedExtrinsic,
		{
			System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
			UseOnceStatements: pallet_use_once_statements::{Pallet, Call, Storage, Event<T>},
		}
	);

	parameter_types! {
		pub const BlockHashCount: u64 = 250;
		pub const MaxDigestsPerBatch: u16 = 10;
		pub const MaxRemoveEntries: u16 = 10;
	}

	impl system::Config for Test {
		type BaseCallFilter = frame_support::traits::Everything;
		type BlockWeights = ();
		type BlockLength = ();
		type DbWeight = ();
		type Origin = Origin;
		type Call = Call;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = Event;
		type BlockHashCount = BlockHashCount;
		type Version = ();
		type PalletInfo = PalletInfo;
		type AccountData = ();
		type OnNewAccount = ();
		type OnKilledAccount = ();
		type SystemWeightInfo = ();
		type SS58Prefix = ();
	}

	impl Config for Test {
		type RuntimeEvent = Event;
		type EnsureOrigin = frame_system::EnsureSigned<u64>;
		type OriginSuccess = frame_system::RawOrigin<u64>;
		type MaxDigestsPerBatch = MaxDigestsPerBatch;
		type MaxRemoveEntries = MaxRemoveEntries;
		type WeightInfo = ();
	}

	#[test]
	fn register_use_once_statement_works() {
		new_test_ext().execute_with(|| {
			let statement_id = 1u64;
			let statement_digest = H256::from_low_u64_be(1);
			let schema_id = 1u64;
			let space_id = 1u64;

			// Register a new use-once statement
			assert_ok!(UseOnceStatements::register(
				Origin::signed(1),
				statement_id,
				statement_digest,
				schema_id,
				space_id
			));

			// Ensure the statement is stored
			assert!(UseOnceStatements::<Test>::contains_key(statement_id));

			// Ensure the statement is marked as used
			assert_eq!(UsedStatements::<Test>::get(statement_id), Some(true));
		});
	}

	#[test]
	fn revoke_use_once_statement_works() {
		new_test_ext().execute_with(|| {
			let statement_id = 1u64;
			let statement_digest = H256::from_low_u64_be(1);
			let schema_id = 1u64;
			let space_id = 1u64;

			// Register a new use-once statement
			assert_ok!(UseOnceStatements::register(
				Origin::signed(1),
				statement_id,
				statement_digest,
				schema_id,
				space_id
			));

			// Revoke the use-once statement
			assert_ok!(UseOnceStatements::revoke(
				Origin::signed(1),
				statement_id,
				statement_digest
			));

			// Ensure the statement is marked as revoked
			assert!(UseOnceRevocationList::<Test>::contains_key(statement_id, statement_digest));
		});
	}

	#[test]
	fn remove_use_once_statement_works() {
		new_test_ext().execute_with(|| {
			let statement_id = 1u64;
			let statement_digest = H256::from_low_u64_be(1);
			let schema_id = 1u64;
			let space_id = 1u64;

			// Register a new use-once statement
			assert_ok!(UseOnceStatements::register(
				Origin::signed(1),
				statement_id,
				statement_digest,
				schema_id,
				space_id
			));

			// Remove the use-once statement
			assert_ok!(UseOnceStatements::remove(
				Origin::signed(1),
				statement_id,
				statement_digest
			));

			// Ensure the statement is removed
			assert!(!UseOnceStatements::<Test>::contains_key(statement_id));
		});
	}

	#[test]
	fn presentation_use_once_statement_works() {
		new_test_ext().execute_with(|| {
			let statement_id = 1u64;
			let statement_digest = H256::from_low_u64_be(1);
			let schema_id = 1u64;
			let space_id = 1u64;
			let presentation_details = UseOnceStatementPresentationDetailsOf::<Test> {
				some_field: vec![1, 2, 3],
				another_field: "Test".into(),
			};

			// Register a new use-once statement
			assert_ok!(UseOnceStatements::register(
				Origin::signed(1),
				statement_id,
				statement_digest,
				schema_id,
				space_id
			));

			// Add a presentation to the use-once statement
			assert_ok!(UseOnceStatements::presentation(
				Origin::signed(1),
				statement_id,
				statement_digest,
				presentation_details.clone()
			));

			// Ensure the presentation is stored
			assert_eq!(
				UseOncePresentations::<Test>::get(statement_id, statement_digest),
				Some(presentation_details)
			);
		});
	}

	#[test]
	fn use_once_statement_cannot_be_used_twice() {
		new_test_ext().execute_with(|| {
			let statement_id = 1u64;
			let statement_digest = H256::from_low_u64_be(1);
			let schema_id = 1u64;
			let space_id = 1u64;

			// Register a new use-once statement
			assert_ok!(UseOnceStatements::register(
				Origin::signed(1),
				statement_id,
				statement_digest,
				schema_id,
				space_id
			));

			// Try to register the same use-once statement again
			assert_noop!(
				UseOnceStatements::register(
					Origin::signed(1),
					statement_id,
					statement_digest,
					schema_id,
					space_id
				),
				Error::<Test>::UseOnceStatementAlreadyUsed
			);

			// Revoke the use-once statement
			assert_ok!(UseOnceStatements::revoke(
				Origin::signed(1),
				statement_id,
				statement_digest
			));

			// Ensure the statement is marked as revoked
			assert!(UseOnceRevocationList::<Test>::contains_key(statement_id, statement_digest));

			// Try to revoke the same use-once statement again
			assert_noop!(
				UseOnceStatements::revoke(Origin::signed(1), statement_id, statement_digest),
				Error::<Test>::UseOnceStatementAlreadyRevoked
			);

			// Remove the use-once statement
			assert_ok!(UseOnceStatements::remove(
				Origin::signed(1),
				statement_id,
				statement_digest
			));

			// Ensure the statement is removed
			assert!(!UseOnceStatements::<Test>::contains_key(statement_id));

			// Try to remove the same use-once statement again
			assert_noop!(
				UseOnceStatements::remove(Origin::signed(1), statement_id, statement_digest),
				Error::<Test>::UseOnceStatementNotFound
			);

			// Ensure the statement is marked as used
			assert_eq!(UsedStatements::<Test>::get(statement_id), Some(true));
		});
	}

	#[test]
	fn issue_and_use_ticket_once() {
		new_test_ext().execute_with(|| {
			let statement_id = 1u64;
			let statement_digest = H256::from_low_u64_be(1);
			let schema_id = 1u64;
			let space_id = 1u64;

			// Issue a new ticket (register a use-once statement)
			assert_ok!(UseOnceStatements::register(
				Origin::signed(1),
				statement_id,
				statement_digest,
				schema_id,
				space_id
			));

			// Use the ticket (e.g., validate or present the statement)
			let presentation_details = UseOnceStatementPresentationDetailsOf::<Test> {
				some_field: vec![1, 2, 3],
				another_field: "Test".into(),
			};
			assert_ok!(UseOnceStatements::presentation(
				Origin::signed(1),
				statement_id,
				statement_digest,
				presentation_details.clone()
			));

			// Ensure the presentation is stored
			assert_eq!(
				UseOncePresentations::<Test>::get(statement_id, statement_digest),
				Some(presentation_details)
			);

			// Attempt to use the ticket again should fail
			assert_noop!(
				UseOnceStatements::presentation(
					Origin::signed(1),
					statement_id,
					statement_digest,
					presentation_details
				),
				Error::<Test>::UseOnceStatementAlreadyUsed
			);
		});
	}

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
		pallet_use_once_statements::GenesisConfig::<Test> {}
			.assimilate_storage(&mut t)
			.unwrap();
		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
