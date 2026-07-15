# This file is part of CORD – https://cord.network

# Copyright (C) Dhiway Networks Pvt. Ltd.
# SPDX-License-Identifier: GPL-3.0-or-later

# CORD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# CORD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with CORD. If not, see <https://www.gnu.org/licenses/>.

"""Deterministic token/structure parsers for the pinned Web3 Storage census.

This deliberately does not pretend regular expressions are an AST.  The lexer
removes comments and preserves balanced syntax, attributes and source offsets;
the parsers then walk declarations, enum/impl/trait bodies and export clauses.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Optional


@dataclass(frozen=True)
class Token:
	value: str
	kind: str
	start: int
	end: int
	line: int


@dataclass(frozen=True)
class Surface:
	kind: str
	symbol: str
	predicate: str
	line: int
	evidence: str = ""


def lex(text: str, nested_comments: bool = True, rust_lifetimes: bool = True) -> list[Token]:
	result: list[Token] = []
	i, line = 0, 1
	while i < len(text):
		char = text[i]
		if char.isspace():
			line += char == "\n"; i += 1; continue
		if text.startswith("//", i):
			end = text.find("\n", i); i = len(text) if end < 0 else end; continue
		if text.startswith("/*", i):
			depth, j = 1, i + 2
			while j < len(text) and depth:
				if nested_comments and text.startswith("/*", j): depth += 1; j += 2; continue
				if text.startswith("*/", j): depth -= 1; j += 2; continue
				line += text[j] == "\n"; j += 1
			i = j; continue
		start, start_line = i, line
		# Rust raw strings, including byte raw strings.
		raw_start = i + 1 if text.startswith("br", i) else i
		if raw_start < len(text) and text[raw_start] == "r":
			j = raw_start + 1
			while j < len(text) and text[j] == "#": j += 1
			if j < len(text) and text[j] == '"':
				marker = '"' + "#" * (j - raw_start - 1); end = text.find(marker, j + 1)
				end = len(text) if end < 0 else end + len(marker)
				value = text[j + 1:end - len(marker)] if end <= len(text) else ""
				line += text[i:end].count("\n"); result.append(Token(value, "string", start, end, start_line)); i = end; continue
		if rust_lifetimes and char == "'" and i + 1 < len(text) and (text[i + 1].isalpha() or text[i + 1] == "_"):
			j = i + 2
			while j < len(text) and (text[j].isalnum() or text[j] == "_"): j += 1
			if j >= len(text) or text[j] != "'":
				result.append(Token("'", "punct", start, i + 1, start_line)); i += 1; continue
		if char in {'"', "'", "`"}:
			quote, j = char, i + 1
			while j < len(text):
				if text[j] == "\\": j += 2; continue
				if text[j] == quote: j += 1; break
				line += text[j] == "\n"; j += 1
			result.append(Token(text[i + 1:j - 1], "string", start, j, start_line)); i = j; continue
		if char.isalpha() or char in {"_", "$"}:
			i += 1
			while i < len(text) and (text[i].isalnum() or text[i] in {"_", "$"}): i += 1
			result.append(Token(text[start:i], "ident", start, i, start_line)); continue
		if char.isdigit():
			i += 1
			while i < len(text) and (text[i].isalnum() or text[i] in {"_", "."}): i += 1
			result.append(Token(text[start:i], "number", start, i, start_line)); continue
		operator = next((op for op in ("::", "=>", "->", "...", "?.", "&&", "||") if text.startswith(op, i)), char)
		i += len(operator); result.append(Token(operator, "punct", start, i, start_line))
	return result


def pairs(tokens: list[Token]) -> tuple[dict[int, int], dict[int, int]]:
	opening, closing, stack = {}, {}, []
	for index, token in enumerate(tokens):
		if token.value in {"(", "[", "{"}: stack.append((token.value, index))
		elif token.value in {")", "]", "}"} and stack:
			want = {")": "(", "]": "[", "}": "{"}[token.value]
			if stack[-1][0] == want:
				_open, start = stack.pop(); opening[start] = index; closing[index] = start
	return opening, closing


def canonical(values: Iterable[str]) -> str:
	return "".join(values)


def canonical_tokens(values: Iterable[Token]) -> str:
	return "".join(json.dumps(token.value) if token.kind == "string" else token.value for token in values)


def attributes_before(tokens: list[Token], index: int, closing: dict[int, int]) -> list[list[Token]]:
	attrs: list[list[Token]] = []
	i = index - 1
	while i >= 1 and tokens[i].value == "]" and i in closing and tokens[closing[i] - 1].value == "#":
		start = closing[i]
		attrs.append(tokens[start + 1:i]); i = start - 2
	attrs.reverse()
	return attrs


def predicate(attrs: list[list[Token]], inherited: str = "") -> str:
	values = []
	for attr in attrs:
		name = canonical_tokens(attr)
		if name.startswith("cfg(") or name.startswith("cfg_attr("):
			values.append(name)
	if inherited and inherited != "always": values.append(inherited)
	return combine_predicates(*values)


def _predicate_parts(value: str) -> list[str]:
	"""Split only top-level conjunctions in a canonical predicate."""
	if not value or value == "always": return []
	parts, start, depth, index = [], 0, 0, 0
	while index < len(value):
		char = value[index]
		if char in "([{" : depth += 1
		elif char in ")]}" : depth -= 1
		elif value.startswith("&&", index) and depth == 0:
			parts.append(value[start:index]); start = index + 2; index += 1
		index += 1
	parts.append(value[start:])
	return [item for item in parts if item]


def combine_predicates(*values: str) -> str:
	"""Combine inherited/item cfg clauses in a stable, order-independent form."""
	parts = sorted({part for value in values for part in _predicate_parts(value)})
	return "&&".join(parts) if parts else "always"


def attr_name(attr: list[Token]) -> str:
	return canonical_tokens(attr).split("(", 1)[0]


def direct_indices(tokens: list[Token], start: int, end: int) -> Iterable[int]:
	depth = 0
	for index in range(start, end):
		if tokens[index].value in {"(", "[", "{"}: depth += 1
		elif tokens[index].value in {")", "]", "}"}: depth -= 1
		elif depth == 0: yield index


def enum_variants(tokens: list[Token], open_index: int, close_index: int, enum_name: str, base_predicate: str, closing: dict[int, int]) -> list[Surface]:
	result, index, at_variant = [], open_index + 1, True
	while index < close_index:
		attrs = attributes_before(tokens, index, closing)
		if at_variant and tokens[index].kind == "ident" and tokens[index].value not in {"pub"}:
			result.append(Surface("rust-enum-variant", f"{enum_name}::{tokens[index].value}", predicate(attrs, base_predicate), tokens[index].line))
			at_variant = False
		if tokens[index].value in {"(", "[", "{"} and index in pairs(tokens)[0]:
			index = pairs(tokens)[0][index]
		elif tokens[index].value == ",": at_variant = True
		index += 1
	return result


def struct_fields(tokens: list[Token], open_index: int, close_index: int, struct_name: str, base_predicate: str, opening: dict[int, int], closing: dict[int, int]) -> list[Surface]:
	result: list[Surface] = []
	for index in direct_indices(tokens, open_index + 1, close_index):
		if tokens[index].value != "pub": continue
		j = index + 1
		if j < close_index and tokens[j].value == "(": j = opening.get(j, j) + 1
		if j + 1 < close_index and tokens[j].kind == "ident" and tokens[j + 1].value == ":":
			result.append(Surface("rust-struct-field", f"{struct_name}::{tokens[j].value}", predicate(attributes_before(tokens, index, closing), base_predicate), tokens[index].line))
	return result


@dataclass(frozen=True)
class RustContext:
	kind: str
	label: str
	open_index: int
	close_index: int
	predicate: str


def _body_after(tokens: list[Token], start: int, limit: int, opening: dict[int, int]) -> Optional[int]:
	"""Return a declaration body opener, stopping at a top-level semicolon."""
	paren_depth = 0
	for index in range(start, limit):
		value = tokens[index].value
		if value in {"(", "["}: paren_depth += 1
		elif value in {")", "]"}: paren_depth -= 1
		elif paren_depth == 0 and value == ";": return None
		elif paren_depth == 0 and value == "{" and index in opening: return index
	return None


def _declaration_start(tokens: list[Token], keyword: int, closing: dict[int, int]) -> int:
	"""Include a visibility token when locating declaration attributes."""
	index = keyword - 1
	if index >= 0 and tokens[index].value == "pub": return index
	if index >= 0 and tokens[index].value == ")" and index in closing:
		open_index = closing[index]
		if open_index >= 1 and tokens[open_index - 1].value == "pub": return open_index - 1
	return keyword


def _skip_impl_generics(values: list[Token]) -> int:
	if not values or values[0].value != "<": return 0
	depth = 0
	for index, token in enumerate(values):
		if token.value == "<": depth += 1
		elif token.value == ">":
			depth -= 1
			if depth == 0: return index + 1
	return 0


def _top_level_keyword(values: list[Token], keyword: str, start: int = 0) -> Optional[int]:
	angle = round_depth = square_depth = 0
	for index in range(start, len(values)):
		value = values[index].value
		if value == "<": angle += 1
		elif value == ">" and angle: angle -= 1
		elif value == "(": round_depth += 1
		elif value == ")" and round_depth: round_depth -= 1
		elif value == "[": square_depth += 1
		elif value == "]" and square_depth: square_depth -= 1
		elif value == keyword and angle == round_depth == square_depth == 0: return index
	return None


def impl_label(tokens: list[Token], impl_index: int, body: int) -> str:
	"""Canonical self/trait identity for an impl block, excluding where clauses."""
	values = tokens[impl_index + 1:body]
	start = _skip_impl_generics(values)
	where = _top_level_keyword(values, "where", start)
	end = len(values) if where is None else where
	for_index = _top_level_keyword(values, "for", start)
	if for_index is None or for_index >= end:
		return canonical_tokens(values[start:end]) or "<anonymous-impl>"
	trait = canonical_tokens(values[start:for_index])
	self_type = canonical_tokens(values[for_index + 1:end])
	return f"<{self_type}as{trait}>"


def _contexts(tokens: list[Token], opening: dict[int, int], closing: dict[int, int]) -> list[RustContext]:
	contexts: list[RustContext] = []
	for index, token in enumerate(tokens):
		if token.value not in {"mod", "trait", "impl"}: continue
		if token.value in {"mod", "trait"} and (index + 1 >= len(tokens) or tokens[index + 1].kind != "ident"):
			continue
		body = _body_after(tokens, index + 1, len(tokens), opening)
		if body is None: continue
		start = _declaration_start(tokens, index, closing)
		own_predicate = predicate(attributes_before(tokens, start, closing))
		label = tokens[index + 1].value if token.value != "impl" else impl_label(tokens, index, body)
		contexts.append(RustContext(token.value, label, body, opening[body], own_predicate))
	return contexts


def _containing_contexts(contexts: list[RustContext], index: int) -> list[RustContext]:
	return sorted(
		(context for context in contexts if context.open_index < index < context.close_index),
		key=lambda context: (context.open_index, -context.close_index),
	)


def _context_identity(contexts: list[RustContext], index: int) -> tuple[str, str, Optional[RustContext]]:
	containing = _containing_contexts(contexts, index)
	modules = [context.label for context in containing if context.kind == "mod"]
	associated = next((context for context in reversed(containing) if context.kind in {"impl", "trait"}), None)
	prefix = "::".join(modules)
	inherited = combine_predicates(*(context.predicate for context in containing))
	return prefix, inherited, associated


def _qualified(prefix: str, symbol: str) -> str:
	return f"{prefix}::{symbol}" if prefix else symbol


def rust_surfaces(text: str) -> list[Surface]:
	tokens = lex(text); opening, closing = pairs(tokens); result: list[Surface] = []
	contexts = _contexts(tokens, opening, closing)
	# Public declarations and public enum variants.
	for index, token in enumerate(tokens):
		if token.value != "pub": continue
		j = index + 1
		if j < len(tokens) and tokens[j].value == "(": j = opening.get(j, j) + 1
		while j < len(tokens) and tokens[j].value in {"async", "unsafe", "const", "extern", "default"}: j += 1
		if j >= len(tokens): continue
		kind = tokens[j].value
		prefix, inherited, associated = _context_identity(contexts, index)
		attrs = attributes_before(tokens, index, closing); pred = predicate(attrs, inherited)
		if kind == "use":
			end = j + 1
			while end < len(tokens) and tokens[end].value != ";": end += 1
			result.append(Surface("rust-public", _qualified(prefix, "use:" + canonical(t.value for t in tokens[j + 1:end])), pred, token.line)); continue
		if kind not in {"fn", "struct", "enum", "trait", "type", "const", "static", "mod"} or j + 1 >= len(tokens): continue
		name = tokens[j + 1].value
		if kind == "fn" and associated is not None:
			symbol = _qualified(prefix, f"{associated.label}::{name}")
		else:
			symbol = _qualified(prefix, f"{kind}:{name}")
		result.append(Surface("rust-public", symbol, pred, token.line))
		if kind == "enum":
			body = _body_after(tokens, j + 2, len(tokens), opening)
			if body is not None:
				result.extend(enum_variants(tokens, body, opening[body], _qualified(prefix, name), pred, closing))
		elif kind == "struct":
			body = _body_after(tokens, j + 2, len(tokens), opening)
			if body is not None:
				result.extend(struct_fields(tokens, body, opening[body], _qualified(prefix, name), pred, opening, closing))
	# Trait members have public API visibility without an explicit pub token.
	for context in contexts:
		if context.kind != "trait": continue
		prefix, inherited, _associated = _context_identity(contexts, context.open_index)
		trait_predicate = combine_predicates(inherited, context.predicate)
		for index in direct_indices(tokens, context.open_index + 1, context.close_index):
			if tokens[index].value != "fn" or index + 1 >= context.close_index: continue
			method_predicate = predicate(attributes_before(tokens, index, closing), trait_predicate)
			result.append(Surface("rust-public", _qualified(prefix, f"{context.label}::{tokens[index + 1].value}"), method_predicate, tokens[index].line))
	# FRAME attributes and individual items.
	for index, token in enumerate(tokens):
		if token.value != "#" or index + 1 >= len(tokens) or tokens[index + 1].value != "[": continue
		end = opening.get(index + 1); attr = tokens[index + 2:end] if end else []
		name = attr_name(attr)
		if not name.startswith("pallet::") or end is None: continue
		j = end + 1
		while j < len(tokens) and tokens[j].value == "#": j = opening.get(j + 1, j) + 1
		section = name.split("::", 1)[1]
		prefix, inherited, _associated = _context_identity(contexts, index)
		if section in {"storage", "constant"}:
			while j < len(tokens) and tokens[j].value not in {"type", "const"}: j += 1
			if j + 1 < len(tokens):
				result.append(Surface(f"frame-{section}", _qualified(prefix, tokens[j + 1].value), predicate([attr], inherited), token.line))
		elif section in {"event", "error"}:
			while j < len(tokens) and tokens[j].value != "enum": j += 1
			if j + 1 < len(tokens):
				body = _body_after(tokens, j + 2, len(tokens), opening)
				if body is not None:
					for item in enum_variants(tokens, body, opening[body], _qualified(prefix, tokens[j + 1].value), predicate([attr], inherited), closing):
						result.append(Surface(f"frame-{section}", item.symbol, item.predicate, item.line))
		elif section in {"call", "hooks"}:
			impl_index = next((k for k in range(j, len(tokens)) if tokens[k].value == "impl"), None)
			body = _body_after(tokens, impl_index + 1, len(tokens), opening) if impl_index is not None else None
			if impl_index is not None and body is not None:
				impl_context = next((context for context in contexts if context.kind == "impl" and context.open_index == body), None)
				impl_identity = impl_context.label if impl_context is not None else impl_label(tokens, impl_index, body)
				impl_predicate = impl_context.predicate if impl_context is not None else predicate(attributes_before(tokens, impl_index, closing))
				base_predicate = combine_predicates(inherited, impl_predicate, predicate([attr]))
				for k in direct_indices(tokens, body + 1, opening[body]):
					if tokens[k].value == "fn" and k + 1 < len(tokens):
						result.append(Surface(
							f"frame-{'call' if section == 'call' else 'hook'}",
							_qualified(prefix, f"{impl_identity}::{tokens[k + 1].value}"),
							predicate(attributes_before(tokens, k, closing), base_predicate),
							tokens[k].line,
						))
	# Runtime API trait and method declarations inside the macro.
	for index, token in enumerate(tokens):
		if token.value != "decl_runtime_apis" or index + 2 >= len(tokens) or tokens[index + 1].value != "!": continue
		body = index + 2
		if tokens[body].value != "{" or body not in opening: continue
		for j in direct_indices(tokens, body + 1, opening[body]):
			if tokens[j].value != "trait" or j + 1 >= len(tokens): continue
			trait_start = _declaration_start(tokens, j, closing)
			trait_attrs = attributes_before(tokens, trait_start, closing)
			prefix, inherited, _associated = _context_identity(contexts, j)
			trait_predicate = predicate(trait_attrs, inherited)
			trait = _qualified(prefix, tokens[j + 1].value); result.append(Surface("runtime-api-trait", trait, trait_predicate, tokens[j].line))
			trait_body = _body_after(tokens, j + 2, opening[body], opening)
			if trait_body is not None and trait_body in opening:
				for k in direct_indices(tokens, trait_body + 1, opening[trait_body]):
					if tokens[k].value == "fn" and k + 1 < len(tokens): result.append(Surface("runtime-api-method", f"{trait}::{tokens[k + 1].value}", predicate(attributes_before(tokens, k, closing), trait_predicate), tokens[k].line))
	# Axum route graph, including multiline handlers.
	for index, token in enumerate(tokens):
		if token.value != "route" or index < 1 or tokens[index - 1].value not in {".", "?."}: continue
		if index + 2 >= len(tokens) or tokens[index + 1].value != "(" or tokens[index + 2].kind != "string": continue
		end = opening.get(index + 1, index + 2); handlers = []
		for k in range(index + 3, min(end, len(tokens))):
			if tokens[k].value in {"get", "post", "put", "delete", "head", "patch"} and k + 2 < end:
				handlers.append(tokens[k].value + ":" + tokens[k + 2].value)
		result.append(Surface("http-route", tokens[index + 2].value + "=>" + ",".join(handlers), "always", token.line))
	return sorted(set(result), key=lambda item: (item.kind, item.symbol, item.predicate, item.line))


def typescript_surfaces(text: str) -> list[Surface]:
	tokens = lex(text, nested_comments=False, rust_lifetimes=False); opening, _closing = pairs(tokens); result: list[Surface] = []
	for index, token in enumerate(tokens):
		if token.value != "export": continue
		j = index + 1
		if j < len(tokens) and tokens[j].value == "default":
			result.append(Surface("typescript-public", "default", "always", token.line)); j += 1
		while j < len(tokens) and tokens[j].value in {"declare", "async", "abstract"}: j += 1
		if j >= len(tokens): continue
		if tokens[j].value == "type" and j + 1 < len(tokens) and tokens[j + 1].value == "{": j += 1
		if tokens[j].value in {"class", "function", "interface", "type", "const", "let", "enum", "namespace"} and j + 1 < len(tokens):
			declaration = tokens[j].value; binding = tokens[j + 1]
			if declaration in {"const", "let"} and binding.value in {"[", "{"} and j + 1 in opening:
				end = opening[j + 1]
				for k in direct_indices(tokens, j + 2, end):
					if tokens[k].kind == "ident": result.append(Surface("typescript-public", f"{declaration}:{tokens[k].value}", "always", token.line))
			else: result.append(Surface("typescript-public", f"{declaration}:{binding.value}", "always", token.line))
			continue
		if tokens[j].value == "*":
			alias = "*"; j += 1
			if j + 1 < len(tokens) and tokens[j].value == "as": alias = tokens[j + 1].value; j += 2
			while j < len(tokens) and tokens[j].value != "from": j += 1
			if j + 1 < len(tokens) and tokens[j + 1].kind == "string": result.append(Surface("typescript-reexport", f"{alias}<-{tokens[j + 1].value}", "always", token.line))
		elif tokens[j].value == "{" and j in opening:
			end = opening[j]; source = "local"
			k = end + 1
			if k + 1 < len(tokens) and tokens[k].value == "from" and tokens[k + 1].kind == "string": source = tokens[k + 1].value
			k = j + 1
			while k < end:
				if tokens[k].value in {",", "type"}: k += 1; continue
				original = tokens[k].value; exported = original; k += 1
				if k + 1 < end and tokens[k].value == "as": exported = tokens[k + 1].value; k += 2
				result.append(Surface("typescript-reexport", f"{exported}<-{source}:{original}", "always", token.line))
				while k < end and tokens[k].value != ",": k += 1
	# Client-to-provider route graph.
	for index, token in enumerate(tokens):
		if token.value != "providerFetch" or index + 1 >= len(tokens) or tokens[index + 1].value != "(": continue
		end = opening.get(index + 1, index + 1)
		strings = [tokens[k].value for k in range(index + 2, min(end, len(tokens))) if tokens[k].kind == "string"]
		if strings: result.append(Surface("typescript-route", strings[0] + "=>providerFetch", "always", token.line))
	return sorted(set(result), key=lambda item: (item.kind, item.symbol, item.predicate, item.line))


def package_export_leaves(value: object, condition: str = "default") -> list[tuple[str, str]]:
	if isinstance(value, str): return [(condition, value)]
	if isinstance(value, list):
		return [item for index, child in enumerate(value) for item in package_export_leaves(child, f"{condition}[{index}]")]
	if isinstance(value, dict):
		return [item for key in sorted(value) for item in package_export_leaves(value[key], f"{condition}.{key}")]
	return []


def package_surfaces(data: bytes) -> list[Surface]:
	value = json.loads(data); result = [Surface("npm-package", str(value.get("name", "private")), "always", 1)]
	exports = value.get("exports", {})
	if isinstance(exports, str): exports = {".": exports}
	if isinstance(exports, dict):
		for subpath in sorted(exports):
			for condition, target in package_export_leaves(exports[subpath]):
				result.append(Surface("npm-export-condition", f"{subpath}:{condition}=>{target}", "always", 1))
	return result
