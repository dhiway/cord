// KILT Blockchain – https://botlabs.org
// Copyright (C) 2019-2021 BOTLabs GmbH

// The KILT Blockchain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The KILT Blockchain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// If you feel like getting in touch with us, you can do so at info@botlabs.org

use sp_std::fmt;

use frame_support::{ensure, BoundedVec};
use sp_std::convert::TryFrom;

use codec::{Decode, Encode};
use sp_std::str;

use crate::{utils, Config, DidError, InputError, UrlError};

/// The expected URI scheme for HTTP endpoints.
pub const HTTP_URI_SCHEME: &str = "http://";
/// The expected URI scheme for HTTPS endpoints.
pub const HTTPS_URI_SCHEME: &str = "https://";
/// The expected URI scheme for FTP endpoints.
pub const FTP_URI_SCHEME: &str = "ftp://";
/// The expected URI scheme for FTPS endpoints.
pub const FTPS_URI_SCHEME: &str = "ftps://";
/// The expected URI scheme for IPFS endpoints.
pub const IPFS_URI_SCHEME: &str = "ipfs://";

/// The content type of a resource pointed by a service URL.
#[derive(Clone, Decode, Debug, Encode, PartialEq, Eq)]
pub enum ContentType {
	/// application/json
	ApplicationJson,
	/// application/json+ld
	ApplicationJsonLd,
}

pub(crate) type UrlPayload<T> = BoundedVec<u8, <T as Config>::MaxUrlLength>;

/// A web URL starting with either http:// or https://
/// and containing only ASCII URL-encoded characters.
#[derive(Clone, Decode, Encode, PartialEq)]
pub struct HttpUrl<T: Config> {
	payload: UrlPayload<T>,
}

impl<T: Config> fmt::Debug for HttpUrl<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("HttpUrl")
			.field("payload", &self.payload.clone().into_inner())
			.finish()
	}
}

impl<T: Config> TryFrom<&[u8]> for HttpUrl<T> {
	type Error = DidError;

	// It fails if the byte sequence does not result in an ASCII-encoded string or
	// if the resulting string contains characters that are not allowed in a URL.
	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
		let str_url = str::from_utf8(value).map_err(|_| UrlError::InvalidUrlEncoding)?;

		ensure!(
			str_url.starts_with(HTTP_URI_SCHEME) || str_url.starts_with(HTTPS_URI_SCHEME),
			UrlError::InvalidUrlScheme
		);

		ensure!(utils::is_valid_ascii_url(str_url), UrlError::InvalidUrlEncoding);

		let payload = BoundedVec::<u8, T::MaxUrlLength>::try_from(value.to_vec())
			.map_err(|_| InputError::MaxUrlLengthExceeded)?;

		Ok(HttpUrl::<T> { payload })
	}
}

/// An FTP URL starting with ftp:// or ftps://
/// and containing only ASCII URL-encoded characters.
#[derive(Clone, Decode, Encode, PartialEq)]
pub struct FtpUrl<T: Config> {
	payload: UrlPayload<T>,
}

impl<T: Config> fmt::Debug for FtpUrl<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("FtpUrl")
			.field("payload", &self.payload.clone().into_inner())
			.finish()
	}
}

impl<T: Config> TryFrom<&[u8]> for FtpUrl<T> {
	type Error = DidError;

	// It fails if the byte sequence does not result in an ASCII-encoded string or
	// if the resulting string contains characters that are not allowed in a URL.
	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
		let str_url = str::from_utf8(value).map_err(|_| UrlError::InvalidUrlEncoding)?;

		ensure!(
			str_url.starts_with(FTP_URI_SCHEME) || str_url.starts_with(FTPS_URI_SCHEME),
			UrlError::InvalidUrlScheme
		);

		ensure!(utils::is_valid_ascii_url(str_url), UrlError::InvalidUrlEncoding);

		let payload = BoundedVec::<u8, T::MaxUrlLength>::try_from(value.to_vec())
			.map_err(|_| InputError::MaxUrlLengthExceeded)?;

		Ok(FtpUrl::<T> { payload })
	}
}

/// An IPFS URL starting with ipfs://. Both CIDs v0 and v1 supported.
#[derive(Clone, Decode, Encode, PartialEq)]
pub struct IpfsUrl<T: Config> {
	payload: UrlPayload<T>,
}

impl<T: Config> fmt::Debug for IpfsUrl<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("IpfsUrl")
			.field("payload", &self.payload.clone().into_inner())
			.finish()
	}
}

impl<T: Config> TryFrom<&[u8]> for IpfsUrl<T> {
	type Error = DidError;

	// It fails if the URL is not ASCII-encoded or does not start with the expected
	// URL scheme.
	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
		let str_url = str::from_utf8(value).map_err(|_| UrlError::InvalidUrlEncoding)?;

		ensure!(str_url.starts_with(IPFS_URI_SCHEME), UrlError::InvalidUrlScheme);

		// Remove the characters of the URL scheme
		let slice_to_verify = str_url
			.get(IPFS_URI_SCHEME.len()..)
			.expect("The minimum length was ensured with starts_with.");

		// Verify the rest are either only base58 or only base32 characters (according
		// to the IPFS specification, respectively versions 0 and 1).
		ensure!(
			utils::is_base_32(slice_to_verify) || utils::is_base_58(slice_to_verify),
			UrlError::InvalidUrlEncoding
		);

		let payload = BoundedVec::<u8, T::MaxUrlLength>::try_from(value.to_vec())
			.map_err(|_| InputError::MaxUrlLengthExceeded)?;

		Ok(IpfsUrl::<T> { payload })
	}
}

/// Supported URLs.
#[derive(Clone, Decode, Debug, Encode, PartialEq)]
pub enum Url<T: Config> {
	/// See [HttpUrl].
	Http(HttpUrl<T>),
	/// See [FtpUrl].
	Ftp(FtpUrl<T>),
	/// See [IpfsUrl].
	Ipfs(IpfsUrl<T>),
}

impl<T: Config> Url<T> {
	#[allow(clippy::len_without_is_empty)]
	pub fn len(&self) -> usize {
		match self {
			Self::Http(HttpUrl::<T> { payload })
			| Self::Ftp(FtpUrl::<T> { payload })
			| Self::Ipfs(IpfsUrl::<T> { payload }) => {
				// We can use .len() as we know the string is ASCII, so 1 byte <-> 1 character
				payload.len()
			}
		}
	}
}

impl<T: Config> From<HttpUrl<T>> for Url<T> {
	fn from(url: HttpUrl<T>) -> Self {
		Self::Http(url)
	}
}

impl<T: Config> From<FtpUrl<T>> for Url<T> {
	fn from(url: FtpUrl<T>) -> Self {
		Self::Ftp(url)
	}
}

impl<T: Config> From<IpfsUrl<T>> for Url<T> {
	fn from(url: IpfsUrl<T>) -> Self {
		Self::Ipfs(url)
	}
}
