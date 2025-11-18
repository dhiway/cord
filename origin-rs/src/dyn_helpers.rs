use crate::error::Error;
use scale_value::Value;
use std::sync::Arc;

/// Helpers for decoding dynamic values using runtime metadata.
pub struct DynHelpers {
	pub layout: Arc<crate::origin_client::RuntimeLayout>,
}

impl DynHelpers {
	pub fn new(layout: Arc<crate::origin_client::RuntimeLayout>) -> Self {
		Self { layout }
	}

	pub async fn decode_as<T: scale_decode::DecodeAsType>(
		&self,
		bytes: &[u8],
		path: &[&str],
	) -> Result<T, Error> {
		self.layout.decode_as_path(bytes, path).await
	}

	pub async fn decode_value_as<T: scale_decode::DecodeAsType>(
		&self,
		value: &Value<u32>,
		path: &[&str],
	) -> Result<T, Error> {
		let type_id = self.layout.type_id_by_path(path).await?;
		let mut bytes = Vec::new();
		scale_value::scale::encode_as_type(value, type_id, self.layout.registry(), &mut bytes)
			.map_err(|e| Error::Codec(e.to_string()))?;
		self.decode_as(&bytes, path).await
	}

    pub fn bytes_from_value(&self, value: &Value<u32>) -> Result<Vec<u8>, Error> {
        match &value.value {
            scale_value::ValueDef::Primitive(p) => {
                if let Some(u) = p.as_u128() {
                    Ok(vec![u as u8])
                } else {
                    Err(Error::Codec("expected byte primitive".into()))
                }
            },
            scale_value::ValueDef::Composite(c) => {
                let mut out = Vec::new();
                match c {
                    scale_value::Composite::Named(fields) => {
                        for (_, v) in fields {
                            out.extend(self.bytes_from_value(v)?);
                        }
                    },
                    scale_value::Composite::Unnamed(values) => {
                        for v in values {
                            out.extend(self.bytes_from_value(v)?);
                        }
                    },
                }
                Ok(out)
            }
            _ => Err(Error::Codec("unsupported byte shape".into())),
        }
    }

    pub fn identifier_hex(&self, bytes: &[u8]) -> String {
        format!("0x{}", hex::encode(bytes))
    }

    pub fn maybe_utf8(&self, bytes: &[u8]) -> Option<String> {
        std::str::from_utf8(bytes).ok().map(|s| s.to_string())
    }
}
