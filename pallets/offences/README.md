# CORD offences pallet

This is a fork of the Substrate `offences` pallet that is modified to agree with the offence rules based on the `authority-member` pallet and not in the Substrate `staking` pallet.

Cord provides a basic way to process offences:

- On offences from `im-online` pallet, the offender disconnection is required.
- On other offences, the offender disconnection is required and the offender is required to be blacklisted and only an authorized origin can remove the offender from the blacklist.

The offences triage is realized in the `offences` pallet and the slashing execution is done in the `authority-member` pallet.
