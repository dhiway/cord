use crate::RegistryIdOf;
use sp_runtime::DispatchError;

/// Allow for more complex schemes on who can attest, revoke and remove.
pub trait StreamAuthorization<CreatorIdOf, SchemaIdOf, AuthorizationIdOf> {
	/// Decides whether the account is allowed to attest with the given
	/// information provided by the sender (&self).
	fn can_create(
		&self,
		who: &CreatorIdOf,
		schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError>;

	/// Decides whether the account is allowed to attest with the given
	/// information provided by the sender (&self).
	fn can_update(
		&self,
		who: &CreatorIdOf,
		schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError>;

	/// Decides whether the account is allowed to revoke the attestation with
	/// the `authorization_id` and the access information provided by the sender
	/// (&self).
	fn can_set_status(
		&self,
		who: &CreatorIdOf,
		schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError>;

	/// Decides whether the account is allowed to remove the attestation with
	/// the `authorization_id` and the access information provided by the sender
	/// (&self).
	fn can_remove(
		&self,
		who: &CreatorIdOf,
		schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError>;

	/// The authorization ID that the sender provided. This will be used for new
	/// attestations.
	///
	/// NOTE: This method must not read storage or do any heavy computation
	/// since it's not covered by the weight returned by `self.weight()`.
	fn authorization_id(&self) -> AuthorizationIdOf;
}

impl<CreatorIdOf, SchemaIdOf, AuthorizationIdOf>
	StreamAuthorization<CreatorIdOf, SchemaIdOf, AuthorizationIdOf> for ()
where
	AuthorizationIdOf: Default,
{
	fn can_create(
		&self,
		_who: &CreatorIdOf,
		_schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError> {
		Err(DispatchError::Other("Unimplemented"))
	}
	fn can_update(
		&self,
		_who: &CreatorIdOf,
		_schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError> {
		Err(DispatchError::Other("Unimplemented"))
	}
	fn can_set_status(
		&self,
		_who: &CreatorIdOf,
		_schema: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError> {
		Err(DispatchError::Other("Unimplemented"))
	}
	fn can_remove(
		&self,
		_who: &CreatorIdOf,
		_ctype: &SchemaIdOf,
	) -> Result<RegistryIdOf, DispatchError> {
		Err(DispatchError::Other("Unimplemented"))
	}
	fn authorization_id(&self) -> AuthorizationIdOf {
		Default::default()
	}
}
