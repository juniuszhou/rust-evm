use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use primitive_types::{H160, H256, U256};
use sha3::{Digest, Keccak256};
use super::{Basic, Backend, ApplyBackend, Apply, Log};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryVicinity {
	pub gas_price: U256,
	pub origin: H160,
	pub chain_id: U256,
	pub block_hashes: Vec<H256>,
	pub block_number: U256,
	pub block_coinbase: H160,
	pub block_timestamp: U256,
	pub block_difficulty: U256,
	pub block_gas_limit: U256,
}

#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub struct MemoryAccount {
	pub nonce: U256,
	pub balance: U256,
	pub storage: BTreeMap<H256, H256>,
	pub code: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct MemoryBackend<'vicinity> {
	vicinity: &'vicinity MemoryVicinity,
	state: BTreeMap<H160, MemoryAccount>,
	logs: Vec<Log>,
}

impl<'vicinity> MemoryBackend<'vicinity> {
	pub fn new(vicinity: &'vicinity MemoryVicinity, state: BTreeMap<H160, MemoryAccount>) -> Self {
		Self {
			vicinity,
			state,
			logs: Vec::new(),
		}
	}

	pub fn state(&self) -> &BTreeMap<H160, MemoryAccount> {
		&self.state
	}
}

impl<'vicinity> Backend for MemoryBackend<'vicinity> {
	fn gas_price(&self) -> U256 { self.vicinity.gas_price }
	fn origin(&self) -> H160 { self.vicinity.origin }
	fn block_hash(&self, number: U256) -> H256 {
		if number >= self.vicinity.block_number ||
			self.vicinity.block_number - number - U256::one() >= U256::from(self.vicinity.block_hashes.len())
		{
			H256::default()
		} else {
			let index = (self.vicinity.block_number - number - U256::one()).as_usize();
			self.vicinity.block_hashes[index]
		}
	}
	fn block_number(&self) -> U256 { self.vicinity.block_number }
	fn block_coinbase(&self) -> H160 { self.vicinity.block_coinbase }
	fn block_timestamp(&self) -> U256 { self.vicinity.block_timestamp }
	fn block_difficulty(&self) -> U256 { self.vicinity.block_difficulty }
	fn block_gas_limit(&self) -> U256 { self.vicinity.block_gas_limit }

	fn chain_id(&self) -> U256 { self.vicinity.chain_id }

	fn exists(&self, address: H160) -> bool {
		self.state.contains_key(&address)
	}

	fn basic(&self, address: H160) -> Basic {
		self.state.get(&address).map(|a| {
			Basic { balance: a.balance, nonce: a.nonce }
		}).unwrap_or_default()
	}

	fn code_hash(&self, address: H160) -> H256 {
		self.state.get(&address).map(|v| {
			H256::from_slice(Keccak256::digest(&v.code).as_slice())
		}).unwrap_or(H256::from_slice(Keccak256::digest(&[]).as_slice()))
	}

	fn code_size(&self, address: H160) -> usize {
		self.state.get(&address).map(|v| v.code.len()).unwrap_or(0)
	}

	fn code(&self, address: H160) -> Vec<u8> {
		self.state.get(&address).map(|v| v.code.clone()).unwrap_or_default()
	}

	fn storage(&self, address: H160, index: H256) -> H256 {
		self.state.get(&address)
			.map(|v| v.storage.get(&index).cloned().unwrap_or(H256::default()))
			.unwrap_or(H256::default())
	}
}

impl<'vicinity> ApplyBackend for MemoryBackend<'vicinity> {
	fn apply<A, I, L>(
		&mut self,
		values: A,
		logs: L,
		delete_empty: bool,
	) where
		A: IntoIterator<Item=Apply<I>>,
		I: IntoIterator<Item=(H256, H256)>,
		L: IntoIterator<Item=Log>,
	{
		for apply in values {
			match apply {
				Apply::Modify {
					address, basic, code, storage, reset_storage,
				} => {
					let is_empty = {
						let account = self.state.entry(address).or_insert(Default::default());
						account.balance = basic.balance;
						account.nonce = basic.nonce;
						if let Some(code) = code {
							account.code = code;
						}

						if reset_storage {
							account.storage = BTreeMap::new();
						}

						let zeros = account.storage.iter()
							.filter(|(_, v)| v == &&H256::default())
							.map(|(k, _)| k.clone())
							.collect::<Vec<H256>>();

						for zero in zeros {
							account.storage.remove(&zero);
						}

						for (index, value) in storage {
							if value == H256::default() {
								account.storage.remove(&index);
							} else {
								account.storage.insert(index, value);
							}
						}

						account.balance == U256::zero() &&
							account.nonce == U256::zero() &&
							account.code.len() == 0
					};

					if is_empty && delete_empty {
						self.state.remove(&address);
					}
				},
				Apply::Delete {
					address,
				} => {
					self.state.remove(&address);
				},
			}
		}

		for log in logs {
			self.logs.push(log);
		}
	}
}
