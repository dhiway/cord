# Tokens

The token pallet maintains the canonical mapping between pallet indexes, event history, and SS58 tokens. Our example touches the pallet in two ways:

1. Every entity, registry, and packet token ultimately flows through the token pallet, so the metadata recorded via `Token::state_event` gives you a deterministic audit trail. After each extrinsic we collect the token from its event and display it alongside a short explanation of the action that produced it.
2. The helper in `flows::dump_packet_state` queries `Register::Packets`, but the `latest_version` value is sourced from the token pallet's state-tracking helpers. Reading the state immediately after submitting an extrinsic is an easy pattern for validating how pallets update their token-backed storage.

The takeaway for developers is that tokens are not an abstract type—Subxt can fetch and pretty-print them exactly like any other runtime data, which makes it trivial to chain identifiers across pallets.
