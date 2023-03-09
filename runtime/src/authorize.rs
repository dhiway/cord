use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::DispatchError;

use pallet_stream::StreamAuthorization;

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum AuthorizationId<AuthorityId> {
	Registry(AuthorityId),
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum PalletAuthorize<AuthorityAc> {
	Registry(AuthorityAc),
}

impl<CreatorIdOf, AuthorityAc, SchemaIdOf, AuthorityId>
	StreamAuthorization<CreatorIdOf, SchemaIdOf, AuthorizationId<AuthorityId>>
	for PalletAuthorize<AuthorityAc>
where
	AuthorityAc: StreamAuthorization<CreatorIdOf, SchemaIdOf, AuthorityId>,
{
	fn can_create(
		&self,
		who: &CreatorIdOf,
		schema: &SchemaIdOf,
	) -> Result<pallet_registry::RegistryIdOf, DispatchError> {
		let PalletAuthorize::Registry(ac) = self;
		ac.can_create(who, schema)
	}

	fn can_update(
		&self,
		who: &CreatorIdOf,
		schema: &SchemaIdOf,
	) -> Result<pallet_registry::RegistryIdOf, DispatchError> {
		let PalletAuthorize::Registry(ac) = self;
		ac.can_update(who, schema)
	}

	fn can_set_status(
		&self,
		who: &CreatorIdOf,
		schema: &SchemaIdOf,
	) -> Result<pallet_registry::RegistryIdOf, DispatchError> {
		let PalletAuthorize::Registry(ac) = self;
		ac.can_set_status(who, schema)
	}

	fn can_remove(
		&self,
		who: &CreatorIdOf,
		schema: &SchemaIdOf,
	) -> Result<pallet_registry::RegistryIdOf, DispatchError> {
		let PalletAuthorize::Registry(ac) = self;
		ac.can_remove(who, schema)
	}

	fn authorization_id(&self) -> AuthorizationId<AuthorityId> {
		let PalletAuthorize::Registry(ac) = self;
		AuthorizationId::Registry(ac.authorization_id())
	}
}
