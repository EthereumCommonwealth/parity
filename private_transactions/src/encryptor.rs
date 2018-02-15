// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Encryption providers.

use std::sync::Arc;
use std::io::Read;
use std::iter::repeat;
use std::time::{Instant, Duration};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use parking_lot::Mutex;
use ethcore::account_provider::AccountProvider;
use ethereum_types::{H128, H256, Address};
use ethjson;
use ethkey::{Signature, Public};
use ethcrypto;
use futures::Future;
use fetch::{Fetch, Client as FetchClient};
use bytes::{Bytes, ToPretty};
use error::PrivateTransactionError;

/// Initialization vector length.
const INIT_VEC_LEN: usize = 16;

/// Duration of storing retrieved keys (in ms)
const ENCRYPTION_SESSION_DURATION: u64 = 30 * 1000;

/// Trait for encryption/decryption operations.
pub trait Encryptor: Send + Sync + 'static {
	/// Generate unique contract key && encrypt passed data. Encryption can only be performed once.
	fn encrypt(
		&self,
		contract_address: &Address,
		accounts: Arc<AccountProvider>,
		initialisation_vector: &H128,
		plain_data: &[u8]
	) -> Result<Bytes, PrivateTransactionError>;

	/// Decrypt data using previously generated contract key.
	fn decrypt(
		&self,
		contract_address: &Address,
		accounts: Arc<AccountProvider>,
		cypher: &[u8]
	) -> Result<Bytes, PrivateTransactionError>;
}

/// Configurtion for key server encryptor
#[derive(Default, PartialEq, Debug, Clone)]
pub struct EncryptorConfig {
	/// URL to key server
	pub base_url: Option<String>,
	/// Key server's threshold
	pub threshold: u32,
	/// Account used for signing requests to key server
	pub key_server_account: Option<Address>,
	/// Passwords used to unlock accounts
	pub passwords: Vec<String>,
}

struct EncryptionSession {
	key: Bytes,
	end_time: Instant,
}

/// SecretStore-based encryption/decryption operations.
pub struct SecretStoreEncryptor {
	config: EncryptorConfig,
	client: FetchClient,
	sessions: Mutex<HashMap<Address, EncryptionSession>>,
}

impl SecretStoreEncryptor {
	/// Create new encryptor
	pub fn new(config: EncryptorConfig) -> Result<Self, PrivateTransactionError> {
		Ok(SecretStoreEncryptor {
			config: config,
			client: FetchClient::new()
				.map_err(|e| PrivateTransactionError::Encrypt(format!("{}", e)))?,
			sessions: Mutex::new(HashMap::new()),
		})
	}

	/// Ask secret store for key && decrypt the key.
	fn retrieve_key(
		&self,
		url_suffix: &str,
		use_post: bool,
		contract_address: &Address,
		accounts: Arc<AccountProvider>
	) -> Result<Bytes, PrivateTransactionError> {
		// check if the key was already cached
		if let Some(key) = self.obtained_key(contract_address) {
			return Ok(key);
		}
		let contract_address_signature = self.sign_contract_address(contract_address, accounts.clone())?;
		let requester = self.config.key_server_account.ok_or_else(|| PrivateTransactionError::KeyServerAccountNotSet)?;

		// key id in SS is H256 && we have H160 here => expand with assitional zeros
		let contract_address_extended: H256 = contract_address.into();
		let base_url = self.config.base_url.clone().ok_or_else(|| PrivateTransactionError::KeyServerNotSet)?;

		// prepare request url
		let url = format!("{}/{}/{}{}",
				base_url,
				contract_address_extended.to_hex(),
				contract_address_signature,
				url_suffix,
			);

		// send HTTP request
		let mut response = match use_post {
			true => self.client.post_with_abort(&url, Default::default()).wait()
				.map_err(|e| PrivateTransactionError::Encrypt(format!("{}", e)))?,
			false => self.client.fetch_with_abort(&url, Default::default()).wait()
				.map_err(|e| PrivateTransactionError::Encrypt(format!("{}", e)))?,
		};

		if response.is_not_found() {
			return Err(PrivateTransactionError::EncryptionKeyNotFound(*contract_address));
		}

		if !response.is_success() {
			return Err(PrivateTransactionError::Encrypt(response.status().canonical_reason().unwrap_or("unknown").into()));
		}

		// read HTTP response
		let mut result = String::new();
		response.read_to_string(&mut result)?;

		// response is JSON string (which is, in turn, hex-encoded, encrypted Public)
		let encrypted_bytes: ethjson::bytes::Bytes = result.parse().map_err(|e| PrivateTransactionError::Encrypt(e))?;

		if let Err(e) = self.unlock_account(&requester, accounts.clone()) {
			trace!("Cannot unlock account: {}", e);
			return Err(PrivateTransactionError::Encrypt(format!("Cannot unlock account {}", e).into()))
		}

		// decrypt Public
		let decrypted_bytes = accounts.decrypt(requester, None, &ethcrypto::DEFAULT_MAC, &encrypted_bytes)?;
		let decrypted_key = Public::from_slice(&decrypted_bytes);

		// and now take x coordinate of Public as a key
		let key: Bytes = (*decrypted_key)[..INIT_VEC_LEN].into();

		// cache the key in the session and clear expired sessions
		self.sessions.lock().insert(*contract_address, EncryptionSession{
			key: key.clone(),
			end_time: Instant::now() + Duration::from_millis(ENCRYPTION_SESSION_DURATION),
		});
		self.clean_expired_sessions();
		Ok(key)
	}

	fn clean_expired_sessions(&self) {
		let mut sessions = self.sessions.lock();
		sessions.retain(|_, session| session.end_time < Instant::now());
	}

	fn obtained_key(&self, contract_address: &Address) -> Option<Bytes> {
		let mut sessions = self.sessions.lock();
		let stored_session = sessions.entry(*contract_address);
		match stored_session {
			Entry::Occupied(session) => {
				if Instant::now() > session.get().end_time {
					session.remove_entry();
					None
				} else {
					Some(session.get().key.clone())
				}
			}
			Entry::Vacant(_) => None,
		}
	}

	/// Try to unlock account using stored passwords
	fn unlock_account(&self, account: &Address, accounts: Arc<AccountProvider>) -> Result<bool, PrivateTransactionError> {
		let passwords = self.config.passwords.clone();
		for password in passwords {
			if let Ok(()) = accounts.unlock_account_temporarily(account.clone(), password) {
				return Ok(true);
			}
		}
		Ok(false)
	}

	fn sign_contract_address(&self, contract_address: &Address, accounts: Arc<AccountProvider>) -> Result<Signature, PrivateTransactionError> {
		// key id in SS is H256 && we have H160 here => expand with assitional zeros
		let contract_address_extended: H256 = contract_address.into();
		let key_server_account = self.config.key_server_account.ok_or_else(|| PrivateTransactionError::KeyServerAccountNotSet)?;
		if let Ok(true) = self.unlock_account(&key_server_account, accounts.clone()) {
			Ok(accounts.sign(key_server_account.clone(), None, H256::from_slice(&contract_address_extended))?)
		} else {
			trace!("Cannot unlock account");
			Err(PrivateTransactionError::Encrypt("Cannot unlock account".into()))
		}
	}
}

impl Encryptor for SecretStoreEncryptor {
	fn encrypt(
		&self,
		contract_address: &Address,
		accounts: Arc<AccountProvider>,
		initialisation_vector: &H128,
		plain_data: &[u8]
	) -> Result<Bytes, PrivateTransactionError> {
		// retrieve the key, try to generate it if it doesn't exist yet
		let key = match self.retrieve_key("", false, contract_address, accounts.clone()) {
			Ok(key) => Ok(key),
			Err(PrivateTransactionError::EncryptionKeyNotFound(_)) => {
				trace!("Key for account wasnt found in sstore. Creating. Address: {:?}", contract_address);
				self.retrieve_key(&format!("/{}", self.config.threshold), true, contract_address, accounts.clone())
			}
			Err(err) => Err(err),
		}?;

		// encrypt data
		let mut cypher = Vec::with_capacity(plain_data.len() + initialisation_vector.len());
		cypher.extend(repeat(0).take(plain_data.len()));
		ethcrypto::aes::encrypt(&key, initialisation_vector, plain_data, &mut cypher);
		cypher.extend_from_slice(&initialisation_vector);

		Ok(cypher)
	}

	/// Decrypt data using previously generated contract key.
	fn decrypt(
		&self,
		contract_address: &Address,
		accounts: Arc<AccountProvider>,
		cypher: &[u8]
	) -> Result<Bytes, PrivateTransactionError> {
		// initialization vector takes INIT_VEC_LEN bytes
		let cypher_len = cypher.len();
		if cypher_len < INIT_VEC_LEN {
			return Err(PrivateTransactionError::Decrypt("Invalid cypher".into()));
		}

		// retrieve existing key
		let key = self.retrieve_key("", false, contract_address, accounts)?;

		// use symmetric decryption to decrypt document
		let (cypher, iv) = cypher.split_at(cypher_len - INIT_VEC_LEN);
		let mut plain_data = Vec::with_capacity(cypher_len - INIT_VEC_LEN);
		plain_data.extend(repeat(0).take(cypher_len - INIT_VEC_LEN));
		ethcrypto::aes::decrypt(&key, &iv, cypher, &mut plain_data);

		Ok(plain_data)
	}
}

/// Dummy encryptor.
#[derive(Default)]
pub struct DummyEncryptor;

impl Encryptor for DummyEncryptor {
	fn encrypt(
		&self,
		_contract_address: &Address,
		_accounts: Arc<AccountProvider>,
		_initialisation_vector: &H128,
		data: &[u8]
	) -> Result<Bytes, PrivateTransactionError> {
		Ok(data.to_vec())
	}

	fn decrypt(
		&self,
		_contract_address: &Address,
		_accounts: Arc<AccountProvider>,
		data: &[u8]
	) -> Result<Bytes, PrivateTransactionError> {
		Ok(data.to_vec())
	}
}

#[cfg(test)]
pub mod tests {
	use super::{Encryptor, DummyEncryptor};
	use rand::{Rng, OsRng};
	use std::sync::Arc;
	use ethereum_types::H128;
	use ethcore::account_provider::AccountProvider;

	const INIT_VEC_LEN: usize = 16;

	fn initialization_vector() -> H128 {
		let mut result = [0u8; INIT_VEC_LEN];
		let mut rng = OsRng::new().unwrap();
		rng.fill_bytes(&mut result);
		H128::from_slice(&result)
	}

	#[test]
	fn dummy_encryptor_works() {
		let encryptor = DummyEncryptor::default();
		let ap = Arc::new(AccountProvider::transient_provider());

		let plain_data = vec![42];
		let iv = initialization_vector();
		let cypher = encryptor.encrypt(&Default::default(), ap.clone(), &iv, &plain_data).unwrap();
		let _decrypted_data = encryptor.decrypt(&Default::default(), ap.clone(), &cypher).unwrap();
	}
}
