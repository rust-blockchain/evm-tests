use crate::mock::{get_state, new_test_ext, setup_state, Runtime};
use crate::utils::*;
use evm_utility::evm::backend::MemoryAccount;
use evm_utility::evm::Config;
use module_evm::{StackExecutor, StackSubstateMetadata, SubstrateStackState, Vicinity};
use primitive_types::H160;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Deserialize, Debug)]
pub struct Test(ethjson::vm::Vm);

impl Test {
	pub fn unwrap_to_pre_state(&self) -> BTreeMap<H160, MemoryAccount> {
		unwrap_to_state(&self.0.pre_state)
	}

	pub fn unwrap_to_vicinity(&self) -> Vicinity {
		Vicinity {
			gas_price: self.0.transaction.gas_price.clone().into(),
			origin: self.0.transaction.origin.clone().into(),
			block_gas_limit: Some(self.0.env.gas_limit.clone().into()),
			block_difficulty: Some(self.0.env.difficulty.clone().into()),
			block_coinbase: Some(self.0.env.author.clone().into()),
		}
	}

	pub fn unwrap_to_code(&self) -> Rc<Vec<u8>> {
		Rc::new(self.0.transaction.code.clone().into())
	}

	pub fn unwrap_to_data(&self) -> Rc<Vec<u8>> {
		Rc::new(self.0.transaction.data.clone().into())
	}

	pub fn unwrap_to_context(&self) -> evm_utility::evm::Context {
		evm_utility::evm::Context {
			address: self.0.transaction.address.clone().into(),
			caller: self.0.transaction.sender.clone().into(),
			apparent_value: self.0.transaction.value.clone().into(),
		}
	}

	pub fn unwrap_to_return_value(&self) -> Vec<u8> {
		self.0.output.clone().unwrap().into()
	}

	pub fn unwrap_to_gas_limit(&self) -> u64 {
		self.0.transaction.gas.clone().into()
	}

	pub fn unwrap_to_post_gas(&self) -> u64 {
		self.0.gas_left.clone().unwrap().into()
	}
}

pub fn test(name: &str, test: Test) {
	new_test_ext().execute_with(|| {
		print!("Running test {} ... ", name);
		flush();

		let original_state = test.unwrap_to_pre_state();
		setup_state(
			original_state,
			test.0.env.number.0.as_u64(),
			test.0.env.timestamp.0.as_u64(),
		);

		let vicinity = test.unwrap_to_vicinity();
		let config = Config::frontier();

		let metadata = StackSubstateMetadata::new(test.unwrap_to_gas_limit(), 1_000_000, &config);
		let state = SubstrateStackState::<Runtime>::new(&vicinity, metadata);
		let mut executor = StackExecutor::new(state, &config);

		let code = test.unwrap_to_code();
		let data = test.unwrap_to_data();
		let context = test.unwrap_to_context();
		let mut runtime = module_evm::evm::Runtime::new(code, data, context, &config);

		let reason = executor.execute(&mut runtime);

		let gas = executor.gas();
		let s = executor.into_state();

		if test.0.output.is_none() {
			print!(
				"Exec reason: {:?} is_succeed: {}",
				reason,
				reason.is_succeed()
			);

			assert!(!reason.is_succeed());
			assert!(test.0.post_state.is_none() && test.0.gas_left.is_none());

			println!("succeed");
		} else {
			let expected_post_gas = test.unwrap_to_post_gas();
			print!(
				"Exec reason: {:?} is_succeed: {}",
				reason,
				reason.is_succeed()
			);

			assert_eq!(
				runtime.machine().return_value(),
				test.unwrap_to_return_value()
			);
			assert_valid_state(test.0.post_state.as_ref().unwrap(), &get_state(&s));
			assert_eq!(gas, expected_post_gas);
			println!("succeed");
		}
	})
}
