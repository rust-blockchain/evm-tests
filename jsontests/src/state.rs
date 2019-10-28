use std::collections::{HashMap, BTreeMap};
use std::rc::Rc;
use serde::Deserialize;
use primitive_types::{H160, H256, U256};
use evm::{Handler, CreateScheme};
use evm::gasometer::{self, Gasometer};
use evm::executor::StackExecutor;
use evm::backend::{Backend, MemoryAccount, MemoryVicinity, MemoryBackend};
use parity_crypto::publickey;
use crate::utils::*;

#[derive(Deserialize, Debug)]
pub struct Test(ethjson::test_helpers::state::State);

impl Test {
	pub fn unwrap_to_pre_state(&self) -> BTreeMap<H160, MemoryAccount> {
		unwrap_to_state(&self.0.pre_state)
	}

	pub fn unwrap_caller(&self) -> H160 {
		let secret_key: H256 = self.0.transaction.secret.clone().unwrap().into();
		let secret = publickey::Secret::import_key(&secret_key[..]).unwrap();
		let public = publickey::KeyPair::from_secret(secret).unwrap().public().clone();
		let sender = publickey::public_to_address(&public);

		sender
	}

	pub fn unwrap_to_vicinity(&self) -> MemoryVicinity {
		MemoryVicinity {
			gas_price: self.0.transaction.gas_price.clone().into(),
			origin: self.unwrap_caller(),
			block_hashes: Vec::new(),
			block_number: self.0.env.number.clone().into(),
			block_coinbase: self.0.env.author.clone().into(),
			block_timestamp: self.0.env.timestamp.clone().into(),
			block_difficulty: self.0.env.difficulty.clone().into(),
			block_gas_limit: self.0.env.gas_limit.clone().into(),
		}
	}
}

pub fn test(name: &str, test: Test) {
	use std::io::{self, Write};

	for (spec, states) in &test.0.post_states {
		let gasometer_config = match spec {
			ethjson::spec::ForkSpec::Istanbul => gasometer::Config::frontier(),
			_ => unimplemented!(),
		};

		let original_state = test.unwrap_to_pre_state();
		let vicinity = test.unwrap_to_vicinity();
		let caller = test.unwrap_caller();

		for (i, state) in states.iter().enumerate() {
			let transaction = test.0.transaction.select(&state.indexes);

			let data: Vec<u8> = transaction.data.into();

			match transaction.to {
				ethjson::maybe::MaybeEmpty::Some(to) => {
					let data = data;
					let value = transaction.value.into();

					let mut backend = MemoryBackend::new(&vicinity, original_state.clone());
					let mut executor = StackExecutor::new(
						&backend,
						transaction.gas_limit.into(),
						&gasometer_config,
					);

					let context = evm::Context {
						address: to.clone().into(),
						caller,
						apparent_value: value,
					};

					executor.transfer(caller, to.clone().into(), value).unwrap();
					executor.call(
						to.clone().into(),
						data,
						Some(transaction.gas_limit.into()),
						false,
						context,
					).unwrap();
				},
				ethjson::maybe::MaybeEmpty::None => {
					let code = data;
					let value = transaction.value.into();

					let mut backend = MemoryBackend::new(&vicinity, original_state.clone());
					let mut executor = StackExecutor::new(
						&backend,
						transaction.gas_limit.into(),
						&gasometer_config,
					);

					let address = executor.create_address(
						caller,
						CreateScheme::Dynamic
					).unwrap();

					let context = evm::Context {
						address,
						caller,
						apparent_value: value,
					};

					executor.transfer(caller, address, value).unwrap();
					executor.create(
						address,
						code,
						Some(transaction.gas_limit.into()),
						context,
					).unwrap();
				},
			}
		}

		println!("vicinity: {:?}", vicinity);
	}
}
