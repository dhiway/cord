# Packets

Packets are versioned records that sit under a registry and must match its schema. The helper `build_packet_attributes` builds a bounded vector of `(Attribute, Element)` pairs, plugs in the maintainer’s entity token, and salts the payload so that repeated runs do not hit duplicate-token errors. After calling `Register::create_packet` we query storage (`Register::Packets`) to prove that the latest version and attribute hash are live on-chain.

Packets also illustrate how Subxt can turn rich runtime types into friendly terminal output. The walkthrough formats each `Elum` value (raw bytes, hashes, or tokens) and prints them next to the stage table so developers can see the payload they just anchored. That makes the module an ideal starting point for pallet tests: drop in new attributes, compare the storage snapshots, and you instantly know whether the pallet behaved as expected.
