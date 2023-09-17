// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

use sp_staking::{offence::OffenceDetails, SessionIndex};

use crate::SlashStrategy;

pub trait OnOffenceHandler<Reporter, Offender, Res> {
	// Required method
	fn on_offence(
		offenders: &[OffenceDetails<Reporter, Offender>],
		slash_strategy: SlashStrategy,
		session: SessionIndex,
	) -> Res;
}
