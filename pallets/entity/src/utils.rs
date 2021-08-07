// use sp_std::str;

use crate::*;

/// Verifies that an input string contains only Base-32 ASCII characters.
/// For more info about what those characters are, please visit the official RFC
/// 4648.
pub fn is_base_32(input: &str) -> bool {
	// Matches [a-z], and [2-7].
	// At the moment, no check is performed the verify that padding characters are
	// only at the end of the char sequence.
	input.chars().all(|c| matches!(c, 'a'..='z' | '2'..='7' | '='))
}

/// Verifies that an input string contains only Base-58 ASCII characters.
/// For more info about what those characters are, please visit the official
/// IETF draft.
pub fn is_base_58(input: &str) -> bool {
	// Matches [A-H], [J-N], [P-Z], [a-k], [m-z], and [1-9].
	input
		.chars()
		.all(|c| matches!(c, 'A'..='H' | 'J'..='N' | 'P'..='Z' | 'a'..='k' | 'm'..='z' | '1'..='9'))
}
