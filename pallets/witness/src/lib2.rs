// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

pub mod types;

pub use crate::{pallet::*, types::*};
use frame_support::{ensure, traits::Get};
use identifier::{
	types::{CallTypeOf, IdentifierTypeOf, Timepoint},
	EventEntryOf,
};
use pallet_chain_space::AuthorizationIdOf;
use sp_runtime::traits::UniqueSaturatedInto;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use cord_primitives::{CountOf, RatingOf};
	use cord_utilities::traits::CallSources;
	use frame_support::{pallet_prelude::*, Twox64Concat};
	use frame_system::pallet_prelude::*;
	pub use identifier::{IdentifierCreator, IdentifierTimeline, IdentifierType, Ss58Identifier};
	use sp_runtime::{
		traits::{Hash, Zero},
		BoundedVec,
	};
	use sp_std::{prelude::Clone, str};

	pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

    // Type of a witness creator identifier.
    pub type WitnessCreatorOf<T> = pallet_chain_space::SpaceCreatorOf<T>;
    // Type of a witness signer identifier.
    pub type WitnessIdOf<T> = pallet_chain_space::SpaceCreatorOf<T>;
    // Type of a document identifier.
    pub type DocumentIdOf = Ss58Identifier;
    //pub type 

    pub type WitnessEntryOf<T> = WitnessEntry<
        WitnessCreatorOf<T>,
        WitnessCountOf<T>,
        BlockNumberFor<T>,
    >;

    pub type WitnessCountOf<T> = <T as Config>::MaxEncodedValueLength;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_chain_space::Config + identifier::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type EnsureOrigin: EnsureOrigin<
			<Self as frame_system::Config>::RuntimeOrigin,
			Success = <Self as Config>::OriginSuccess,
		>;
		type OriginSuccess: CallSources<AccountIdOf<Self>, WitnessCreatorOf<Self>>;
		#[pallet::constant]
		type MaxEncodedValueLength: Get<u32>;

        #[pallet::constant]
        type MaxWitnessCount: Get<u32>;

		//type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    /// Witness entry identifiers with details stored on chain.
    #[pallet::storage]
    pub type Witness<T> = StorageMap<_, Blake2_128Concat, DocumentIdOf, WitnessEntryOf<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {

        /// A new witness entry has been added.
        /// \[document entry identifier, issuer\]
        Create { identifier: DocumentIdOf, issuer: WitnessCreatorOf<T> },

        /// Witness signs the document
        /// \[ document entry identifier, witness\]
        // todo: change below to witness_entry_id, signer
        Witness { identifier: DocumentIdOf, witness: WitnessIdOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Unauthorized operation
		UnauthorizedOperation,
        /// Witness creation already added
        WitnessIdAlreadyExists,
        /// Witness Identifier is already approved,
        WitnessIdAlreadyApproved,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {

        /** create(doc_identifier, NoOfWitnesses)
         *   - check if the doc_id exists?
         *   - create a identifier for the create witness by the combination of (doc_identifier, caller of create, chainspace_id?)
         *   - store who is calling create
         *   - set status to `Witness Pending` for the doc_identifier in storage, 
         *   - returns a witness entry creation identifier, for future witness approvals.
         *   - Update Activitiy, Send Event Ok()
         */

        /** witness(doc_id, array_of_witnesss/ individual_witness)
          * - check if the doc_id exists
          * - check if the witness_id is already approved state or not, if yes return error.
          * - check if the number of witness signing count is same of required, but the array of witness should be other than creator
          * - set status to `Witness Sign Complete' for the doc_identifier
          * - Update Activity, Send Event Ok()
         */

         // let witness_signers =
			// 	<WitnessesSignatures<T>>::get(&identifier).ok_or(Error::<T>::WitnessIdNotFound)?;
            
            // if witness_signers.witnesses is None or witness_signers.witnesses.len() == 0 {
            //     // Insert signer to the bounded vec
            //     <WitnessesSignatures<T>>::insert(
            //         &identifier,
            //         WitnessSignersEntryOf::<T> {
            //             witnesses: // insert here
            //             created_at: block_number
            //         },
            //     );
            // } else {
            //     // Iterate over all existing signers in WitnessSignatures storage
            //     // If the current signer exists, throw error,
            //     // Else append the current signer to the list of witnesses
            // }



            //let witness_signers = Some(witness_entry.witness_signers);

            /* If no witness exist currently, add without validation */
                // if witness_entry.witness_signers.len() == 0 {
                //    witness_entry.witness_signers.push(Some(signer.clone()));
                // } 
                

            // Else 
                // /* Validate if signer is not already a witness */



                // /* Add witness signers to a vec */
                // match witnesses.push(signer.clone()) {
                //     Ok(()) => {
                //         // Witness added successfully
                //         // Proceed with the rest of the logic
                //     }
                //     Err(_) => {
                //         // Handle the case where the array is already at maximum capacity
                //         // This would occur if the number of existing witnesses is already equal to `MaxWitnessCount`
                //         return Err(Error::<T>::MaxWitnessCountReached.into());
                //     }
                // }


        #[pallet::call_index(0)]
        #[pallet::weight({0})]
        pub fn create(
            origin: OriginFor<T>,
            doc_identifer: DocumentIdOf,
            witness_count: WitnessCountOf<T>,
            authorization: AuthorizationIdOf,
        ) -> DispatchResult {
            let creator = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
            let space_id = pallet_chain_space::<T>::ensure_authorization_origin(
                &authorization,
                &creator.clone(),
            ).map_err(<pallet_chain_space::Error<T>>::from)?;

            let id_digest = <T as frame_system::Config>::Hashing::hash(
                &[&doc_identifer.encode()[..], &space_id.encode()[..], &creator.encode()[..]]
            );

            let identifier =
				Ss58Identifier::create_identifier(&(id_digest).encode()[..], IdentifierType::Witness)
					.map_err(|_| Error::<T>::InvalidIdentifierLength)?;

			ensure!(!<Witness<T>>::contains_key(&identifier), Error::<T>::WitnessIdAlreadyExists);

			let block_number = frame_system::Pallet::<T>::block_number();

            <Witness<T>>::insert(
				&identifier,
				WitnessEntryOf::<T> {
                    witness_creator: creator.clone(),
                    witness_signature_required: witness_count,
                    witness_status: WitnessStatusOf::WITNESSAPRROVALPENDING,
					created_at: block_number,
				},
			);

            Self::update_activity(&identifier, CallTypeOf::Genesis).map_err(<Error<T>>::from)?;
			Self::deposit_event(Event::Create { identifier, issuer: creator });

			Ok(())
        }

        // #[pallet::call_index(1)]
        // #[pallet::weight({0})]
        // pub fn witness(
        //     origin: OriginFor<T>,
        //     identifier: DocumentIdOf,
        //     //Option<witness_signer: WitnessIdOf<T>>,
        // ) -> DispatchResult {
        //     let witness_signer = <T as Config>::EnsureOrigin::ensure_origin(origin)?.subject();
        //     let space_id = pallet_chain_space::Pallet::<T>::ensure_authorization_origin(
        //         &authorization,
        //         &creator.clone(),
        //     ).map_err(<pallet_chain_space::Error<T>>::from)?;

        //     let witness_entry = <Witness<T>>::get(&).ok_or(Error::<T>::WitnessIdNotFound)?;
            
        //     /* Individual who created the witness entry record cannot be part of witness signer party */
        //     ensure!(witness_entry.witness_creator == witness_signer, Error::<T>::UnauthorizedOperation);

        //     Ok(())
        // }
	}
}

impl<T: Config> Pallet<T> {
	// pub fn get_distributed_qty(asset_id: &AssetIdOf) -> u32 {
	// 	<Distribution<T>>::get(asset_id)
	// 		.map(|bounded_vec| bounded_vec.len() as u32)
	// 		.unwrap_or(0)
	// }

	pub fn update_activity(tx_id: &DocumentIdOf, tx_action: CallTypeOf) -> Result<(), Error<T>> {
		let tx_moment = Self::timepoint();

		let tx_entry = EventEntryOf { action: tx_action, location: tx_moment };
		let _ = IdentifierTimeline::update_timeline::<T>(tx_id, IdentifierTypeOf::Witness, tx_entry);
		Ok(())
	}

	pub fn timepoint() -> Timepoint {
		Timepoint {
			height: frame_system::Pallet::<T>::block_number().unique_saturated_into(),
			index: frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
		}
	}
}
