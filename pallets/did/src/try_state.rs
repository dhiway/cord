// This file is part of CORD – https://cord.network

// Copyright (C) 2019-2023 BOTLabs GmbH.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

use cord_utilities::test_utils::log_and_return_error_message;
use frame_support::ensure;
use scale_info::prelude::format;
use sp_core::Get;
use sp_runtime::{SaturatedConversion, TryRuntimeError};

use crate::{
	did_details::DidDetails, Config, Did, DidBlacklist, DidEndpointsCount, DidIdentifierOf,
	ServiceEndpoints,
};

#[allow(dead_code)]
pub(crate) fn do_try_state<T: Config>() -> Result<(), TryRuntimeError> {
	Did::<T>::iter().try_for_each(
		|(did_subject, did_details): (DidIdentifierOf<T>, DidDetails<T>)| -> Result<(), TryRuntimeError> {
			let service_endpoints_count = ServiceEndpoints::<T>::iter_prefix(&did_subject).count();

			ensure!(
				service_endpoints_count == DidEndpointsCount::<T>::get(&did_subject).saturated_into::<usize>(),
				log_and_return_error_message(format!(
					"Did {:?} has not matching service endpoints. In [ServiceEndpoints]: {:?} in [DidEndpointsCount]: {:?}",
					did_subject,
					service_endpoints_count,
					DidEndpointsCount::<T>::get(&did_subject)
				))
			);

			ensure!(
				did_details.key_agreement_keys.len()
					<= (<T as Config>::MaxTotalKeyAgreementKeys::get()).saturated_into::<usize>(),
				log_and_return_error_message(format!(
					"Did {:?} has to many key agreement keys. Allowed: {:?} found: {:?}",
					did_subject,
					<T as Config>::MaxTotalKeyAgreementKeys::get(),
					did_details.key_agreement_keys.len()
				))
			);

			ensure!(
				service_endpoints_count <= <T as Config>::MaxNumberOfServicesPerDid::get().saturated_into::<usize>(),
				log_and_return_error_message(format!(
					"Did {:?} has to many service endpoints. Allowed: {:?} found: {:?}",
					did_subject,
					<T as Config>::MaxNumberOfServicesPerDid::get(),
					service_endpoints_count
				))
			);

			ensure!(
				!DidBlacklist::<T>::contains_key(&did_subject),
				log_and_return_error_message(format!("Did {:?} is blacklisted.", did_subject))
			);

			Ok(())
		},
	)?;

	DidBlacklist::<T>::iter_keys().try_for_each(
		|deleted_did_subject| -> Result<(), TryRuntimeError> {
			let service_endpoints_count =
				ServiceEndpoints::<T>::iter_prefix(&deleted_did_subject).count();
			ensure!(
				service_endpoints_count == 0,
				log_and_return_error_message(format!(
					"Blacklisted did {:?} has service endpoints.",
					deleted_did_subject,
				))
			);
			Ok(())
		},
	)
}
