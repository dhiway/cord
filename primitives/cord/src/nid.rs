/ use base58::{FromBase58, ToBase58};
// use codec::{Decode, Encode};
// use scale_info::TypeInfo;
// use serde::{Deserialize, Deserializer, Serialize, Serializer};
// use sp_std::str::FromStr;
//
// #[derive(Debug, Clone, PartialEq, Eq, Hash, Encode, Decode)]
// #[cfg_attr(feature = "std", derive(TypeInfo,))]
// pub struct NodeId(pub(crate) String);
//
// impl Serialize for NodeId {
// 	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
// 	where
// 		S: Serializer,
// 	{
// 		serializer.serialize_str(&self.0)
// 	}
// }
//
// impl<'de> Deserialize<'de> for NodeId {
// 	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
// 	where
// 		D: Deserializer<'de>,
// 	{
// 		let s = String::deserialize(deserializer)?;
// 		NodeId::from_str(&s).map_err(serde::de::Error::custom)
// 	}
// }
//
// impl NodeId {
// 	// Create a new NodeId from a base58 encoded string
// 	pub fn new(encoded: &str) -> Result<Self, &'static str> {
// 		Self::validate_base58(encoded)?;
// 		Ok(NodeId(encoded.to_string()))
// 	}
//
// 	// Validate if the string is a valid base58 encoded string
// 	fn validate_base58(encoded: &str) -> Result<(), &'static str> {
// 		let decoded = encoded.from_base58().map_err(|_| "Invalid Format")?;
// 		let re_encoded = decoded.to_base58();
// 		if re_encoded != encoded {
// 			return Err("Invalid Identifier")
// 		}
// 		Ok(())
// 	}
// 	// Method to get the length of the internal String
// 	pub fn len(&self) -> usize {
// 		self.0.len()
// 	}
//
// 	// Method to check if the internal String is empty
// 	pub fn is_empty(&self) -> bool {
// 		self.0.is_empty()
// 	}
//
// 	// Method to get the base58 string representation
// 	pub fn to_base58(&self) -> String {
// 		self.0.clone()
// 	}
// }
//
// impl FromStr for NodeId {
// 	type Err = &'static str;
//
// 	fn from_str(s: &str) -> Result<Self, Self::Err> {
// 		if s.len() != 53 {
// 			return Err("Invalid Identifier Length")
// 		}
// 		Self::validate_base58(s)?;
// 		Ok(NodeId(s.to_string()))
// 	}
// }
//
// impl AsRef<[u8]> for NodeId {
// 	fn as_ref(&self) -> &[u8] {
// 		self.0.as_bytes()
// 	}
// }
//
// #[cfg(test)]
// mod tests {
// 	use super::NodeId;
// 	use std::str::FromStr;
//
// 	#[test]
// 	fn valid_node_id() {
// 		let id_str = "12D3KooWMURLFvqEKrZucCvKcA7wrMgwd4UB681RLsfQCiQAPvK4";
// 		let node_id = NodeId::from_str(id_str).expect("Should be valid");
// 		assert_eq!(node_id.to_base58(), id_str);
// 	}
//
// 	#[test]
// 	fn invalid_node_id() {
// 		let id_str = "invalid_length";
// 		let node_id = NodeId::from_str(id_str);
// 		assert!(node_id.is_err());
// 	}
// }
