# Element Mapping Cheatsheet

- Raw: JSON value is serialized as base64-encoded bytes; useful for nested objects.  
- Bool: JSON `true/false` → Element::Bool.  
- U64: JSON number → Element::U64.  
- U128: JSON number or string → Element::U128.  
- Hash: hex string (32-byte) → Element::Hash.  
- Token: ss58 string → Element::Token.  
- Cid: ASCII string (8–128 chars) → Element::Cid.

When reading, `ElementView` is rendered back to JSON using the same rules (hash as hex, token as ss58 string, raw as base64 string).

Batch attribute updates: supply a JSON object and a `(key, ElementType, optional)` schema slice; the SDK builds `Element` variants and submits rotate_attribute calls, preserving view-only reads and avoiding storage RPCs.
