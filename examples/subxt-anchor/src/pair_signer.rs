use sp_core::{sr25519, Pair as CryptoPair};
use sp_runtime::{
    traits::{IdentifyAccount, Verify},
    MultiSignature as SpMultiSignature,
};
use subxt::{
    config::substrate::{AccountId32, MultiSignature},
    tx::Signer,
};

use crate::chain::CordConfig;

#[derive(Clone)]
pub struct PairSigner {
    account_id: <CordConfig as subxt::config::Config>::AccountId,
    signer: sr25519::Pair,
}

impl PairSigner {
    pub fn new(signer: sr25519::Pair) -> Self {
        let account_id = <SpMultiSignature as Verify>::Signer::from(signer.public()).into_account();
        Self { account_id: AccountId32(account_id.into()), signer }
    }

    pub fn account_id(&self) -> &AccountId32 {
        &self.account_id
    }
}

impl Signer<CordConfig> for PairSigner {
    fn account_id(&self) -> <CordConfig as subxt::config::Config>::AccountId {
        self.account_id.clone()
    }

    fn sign(&self, signer_payload: &[u8]) -> <CordConfig as subxt::config::Config>::Signature {
        let signature = self.signer.sign(signer_payload);
        MultiSignature::Sr25519(signature.0)
    }
}
