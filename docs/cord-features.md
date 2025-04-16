# CORD Features

In this document, we are trying to give elaborate understanding of what each of pallet signifies, and how it would keep proofs in them. Idea is to make sure everyone, mainly developers understand how to make sense of the 'Token' and understand how it would be linked to all the other tokens, and their relations.

![image](https://hackmd.io/_uploads/SktR5aoaJg.png)


### Identifier

The identifier in CORD chain looks like below:

`2L1sasM85CBtLkxFjB1ADWc9erKr4xFuALh87vkMEJbQfVVwM5zGTdGTPV`

The spec of the Identifier is available at [Spec Repository](https://github.com/dhiway/specs/blob/main/01-Identifier.md)

Identifier provides query methods to keep track of all updates to identifier (which has to be updated by every pallet dealing with the identifier). Also note that every identifier by itself can have a state (Active/Revoked).


### profile

This is a pallet which depends on [Identifier](#identifier) to create the profile-identifier. This pallet is mainly designed to handle key rotations and public key management for any identity in CORD chain.

The historic key tracking is offloaded to services which can be built by indexers of chain's blocks.

In this pallet, we allow account to create key-value pairs (like any other profiles). The pallet is not checking for the value, and the applications using profile should take care of DPDP / GDPR like laws.

This pallet doesn't 'recommend' (ie, enforce) the PII data check right now. Plan is, in future if there is a need, this pallet can be changed to take only hash as value for any key, and that hash should be resolved by attached storage.

### registry

This pallet handles 2 things. One is to create a identifier which would be used to group certain proofs, and second is handling the delegations. Using this pallet, we can setup a small subset of chain, which will accept the proofs only from the accounts which are added as the delegates, thus making updating any existing proofs possible only from valid proof, and not any anonymous account which has some UNITs/WAYs to pay for chain transaction. This key feature enables the trust in registries, as the process of management of delegates itself is recorded on the ledger. Note that this pallet depends on [profile](#profile) pallet to manage the delegates, and on [identifier](#identifier) feature for identifier creation and updation.

Before creating a registry, the profile should be created by the account.

### entry

This pallet can by synonyms with record, or statement (ie, a previous pallet used for 'proof'). This depends on profile and registry pallet to work. To call any extrinsic (or method/function) related to entry pallet, the account should have created profile, and a registry.

An entry can be created for a single digest, or for a blob (where no data check doesn't happen), or a storage id (ie, nodeid+content-id combination). Any of these calls will be creating the identifier. Once Identifier is created, we can attach some more 'key-value' data to identifier thus applications can be built with more data points to verify the proofs.

Note that any entry can also be revoked by the relevant account, and thus the chain reflects the latest state of the entry.

### collection

A 'collection' as the name suggests, is a collection of 'identifiers'. A collection also has an identifier, thus its can manage its state/status, also be watched by applications. The real-world example of collection is a playlist, where one keeps multiple entries which are not created by them, but associates them based on their own tags, groups etc. Similarly, each collection here can be an array of identifiers which will be grouped together by one of the profile.

This enables a country to provide set of 'registry-identifiers' as the registry to consider for all validations, or a user to use proof of all credentials used as part of a verifiable presentation. Again, the usecases of how best to use colleciton is left to applications. One thing we have considered is not to check for existance of this identifier in the same chain, because a collection can have identifiers from multiple chains.


## Other highlights, and future developments

### rating

This can be a separate pallet, which doesn't create any new identifier, but can keep track of rating of any identifier? Questions asked are:

* Can every identifier have rating a common place like status (active/revoke) ?
* This can enable rating for a profile, a registry or an entry.

### contracts

Smart Contracts are key factor in blockchain adoption. In this scenario, the applications can trust the 'binary' confidence that they are executing the logic which is seen by everyone, and the underlaying code has not changed behind the scene. It is good enough for trust in multiple domains.

Today Ethereum's smart contract is considered as widely adopted, hence CORD also would support the EVM friendly Smart Contracts. Thus, a user can use the same infrastructure they are using for trust infrastructure for their application's smart contracts too.

This space will be evolved over time, and we will keep updating the docs on this.

### witness

When we consider data trust, in the real-world, there is a concept of 'witness' while registering a document, in legal domain etc. Thus, we can develop a simple pallet, which doesn't create a new identifier, but adds profile identifier which has done the`witness()` call.

## How to understand which is good for me?

Think, document and discuss with us. We are happy how your application can benefit from this Data Tokenization enabling pallets / features.

