use crate::cord::runtime_types::cord_primitives::{element::Elum, identifier::Ss58Identifier};
use crate::view_types::IdentifierView;
use hex::encode as hex_encode;

pub trait IdentifierLike {
	fn raw_bytes(&self) -> &[u8];
}

impl IdentifierLike for IdentifierView {
	fn raw_bytes(&self) -> &[u8] {
		self.as_bytes()
	}
}

impl IdentifierLike for Ss58Identifier {
	fn raw_bytes(&self) -> &[u8] {
		&(self.0).0
	}
}

pub fn token_to_string<T: IdentifierLike>(token: &T) -> String {
	String::from_utf8(token.raw_bytes().to_vec()).unwrap_or_else(|_| "<invalid>".into())
}

pub fn describe_element(element: &Elum) -> String {
	match element {
		Elum::None => "None".into(),
		Elum::Raw(data) => {
			let bytes = data.0.clone();
			format!("Raw({})", String::from_utf8_lossy(&bytes))
		},
		Elum::Bool(flag) => format!("Bool({flag})"),
		Elum::U64(bytes) => format!("U64({})", u64::from_le_bytes(*bytes)),
		Elum::U128(bytes) => format!("U128({})", u128::from_le_bytes(*bytes)),
		Elum::Hash(digest) => format!("Hash(0x{})", hex_encode(digest)),
		Elum::Token(id) => format!("Token({})", token_to_string(id)),
		Elum::CID(cid) => {
			let data = cid.0.clone();
			format!("CID({})", hex_encode(data))
		},
	}
}
