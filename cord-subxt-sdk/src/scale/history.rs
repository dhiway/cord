use crate::error::{Error, Result};
use cord_primitives::{
	dev::DevEventBlockView,
	view::{DevElement, InfoAttributeHistoryEntry},
};
use scale_value::{Composite, Value, ValueDef, Variant};
use std::collections::BTreeMap;

fn flatten<'a>(value: &'a Value<u32>) -> &'a Value<u32> {
    match &value.value {
        ValueDef::Composite(Composite::Named(fields)) if fields.len() == 1 => flatten(&fields[0].1),
        ValueDef::Composite(Composite::Unnamed(items)) if items.len() == 1 => flatten(&items[0]),
        _ => value,
    }
}

fn variant<'a>(value: &'a Value<u32>) -> Option<&'a Variant<u32>> {
    match &flatten(value).value {
        ValueDef::Variant(v) => Some(v),
        _ => None,
    }
}

fn first_field<'a>(composite: &'a Composite<u32>) -> Option<&'a Value<u32>> {
    match composite {
        Composite::Named(fields) => fields.first().map(|(_, v)| v),
        Composite::Unnamed(items) => items.first(),
    }
}

fn option_inner<'a>(value: &'a Value<u32>) -> Option<&'a Value<u32>> {
    match variant(value) {
        Some(var) if var.name == "None" => None,
        Some(var) if var.name == "Some" => first_field(&var.values),
        _ => Some(value),
    }
}

fn value_bytes(value: &Value<u32>) -> Option<Vec<u8>> {
    match &flatten(value).value {
        ValueDef::Composite(Composite::Unnamed(items))
            if items.iter().all(|item| item.as_u128().is_some()) =>
        {
            Some(items.iter().map(|item| item.as_u128().unwrap() as u8).collect())
        }
        ValueDef::Variant(var) => first_field(&var.values).and_then(value_bytes),
        ValueDef::Primitive(_) => value.as_u128().map(|n| vec![n as u8]),
        ValueDef::BitSequence(bits) => {
            let mut out = Vec::new();
            let mut accum = 0u8;
            let mut count = 0;
            for bit in bits.iter() {
                if bit {
                    accum |= 1 << count;
                }
                count += 1;
                if count == 8 {
                    out.push(accum);
                    accum = 0;
                    count = 0;
                }
            }
            if count > 0 {
                out.push(accum);
            }
            Some(out)
        }
        _ => None,
    }
}

fn decode_block(value: &Value<u32>) -> Result<DevEventBlockView> {
    if let ValueDef::Composite(Composite::Named(fields)) = &value.value {
        let mut height = None;
        let mut index = None;
        for (name, field) in fields {
            match name.as_str() {
                "height" => height = field.as_u128().map(|v| v as u32),
                "index" => index = field.as_u128().map(|v| v as u32),
                _ => {},
            }
        }
        let height = height.ok_or_else(|| Error::Codec("missing block height".into()))?;
        let index = index.ok_or_else(|| Error::Codec("missing block index".into()))?;
        Ok(DevEventBlockView { height, index })
    } else {
        Err(Error::Codec("invalid block structure".into()))
    }
}

fn decode_dev_element(value: &Value<u32>) -> Result<DevElement> {
    use cord_primitives::view::DevElement;
    let var = variant(value).ok_or_else(|| Error::Codec("expected dev element".into()))?;
    let field = first_field(&var.values);
    match var.name.as_str() {
        "None" => Ok(DevElement::None),
        "Bool" => Ok(DevElement::Bool(field.and_then(|f| f.as_u128()).map(|n| n != 0).unwrap_or(false))),
        "U64" => {
            let bytes = value_bytes(field.ok_or_else(|| Error::Codec("missing u64".into()))?)?
                .try_into()
                .map_err(|_| Error::Codec("invalid u64".into()))?;
            Ok(DevElement::U64(u64::from_le_bytes(bytes)))
        }
        "U128" => {
            let bytes = value_bytes(field.ok_or_else(|| Error::Codec("missing u128".into()))?)?
                .try_into()
                .map_err(|_| Error::Codec("invalid u128".into()))?;
            Ok(DevElement::U128(u128::from_le_bytes(bytes)))
        }
        "HashHex" | "Hash" => Ok(DevElement::HashHex(
            value_bytes(field.ok_or_else(|| Error::Codec("missing hash".into()))?)
                .map(|bytes| format!("0x{}", hex::encode(bytes)))
                .unwrap_or_default(),
        )),
        "TokenSs58" | "Token" => Ok(DevElement::TokenSs58(
            value_bytes(field.ok_or_else(|| Error::Codec("missing token".into()))?)
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .unwrap_or_default(),
        )),
        "CidBase58" | "CID" => Ok(DevElement::CidBase58(
            value_bytes(field.ok_or_else(|| Error::Codec("missing cid".into()))?)
                .map(|bytes| bs58::encode(bytes).into_string())
                .unwrap_or_default(),
        )),
        "RawBase64" | "Raw" => Ok(DevElement::RawBase64(
            value_bytes(field.ok_or_else(|| Error::Codec("missing raw".into()))?)
                .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
                .unwrap_or_default(),
        )),
        other => Err(Error::Codec(format!("unsupported dev element variant {other}"))),
    }
}

fn decode_entry(value: &Value<u32>) -> Result<InfoAttributeHistoryEntry> {
    if let ValueDef::Composite(Composite::Named(fields)) = &value.value {
        let mut key_hex = None;
        let mut key_utf8 = None;
        let mut version = None;
        let mut old_value = None;
        let mut block = None;
        for (name, field) in fields {
            match name.as_str() {
                "key_hex" => key_hex = field.as_str().map(str::to_string),
                "key_utf8" => key_utf8 = option_inner(field).and_then(|v| v.as_str()).map(str::to_string),
                "version" => version = field.as_u128().map(|v| v as u64),
                "old_value" | "oldValue" => {
                    let dev = decode_dev_element(field)?;
                    old_value = Some(dev);
                }
                "block" => block = Some(decode_block(field)?),
                "old_value_base64" | "oldValueBase64" => {
                    if key_hex.is_none() {
                        key_hex = field.as_str().map(str::to_string);
                    }
                }
                _ => {},
            }
        }
        let key_hex = key_hex.ok_or_else(|| Error::Codec("missing key_hex".into()))?;
        let version = version.ok_or_else(|| Error::Codec("missing version".into()))?;
        let dev_value = old_value.ok_or_else(|| Error::Codec("missing old value".into()))?;
        let block = block.ok_or_else(|| Error::Codec("missing block".into()))?;
        let rendered = crate::scale::value::dev_element_to_string(dev_value.clone());
        Ok(InfoAttributeHistoryEntry {
            key_hex,
            key_utf8,
            version,
            old_value_base64: rendered,
            block,
        })
    } else {
        Err(Error::Codec("invalid history entry".into()))
    }
}

pub fn decode_history(entries: &Value<u32>) -> Result<Vec<InfoAttributeHistoryEntry>> {
    let Some(items) = super::value::sequence_items(entries) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(items.len());
    for entry in items {
        out.push(decode_entry(entry)?);
    }
    Ok(out)
}
